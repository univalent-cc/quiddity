"""Identities as data (spec 5): :class:`Identity`."""

from __future__ import annotations

import copy
import json
from collections.abc import Mapping
from typing import Any, ClassVar, Final

from . import _declared, _load, _value
from ._apply import Applier
from ._digest import Digest
from ._errors import InvalidIdentity, QuiddityError
from ._kinds import Builder, _optional
from ._load import Defs, Kind
from ._markers import (
    AsSet,
    Decimals,
    FieldMarker,
    OmitEmpty,
    OmitNone,
    Quantize,
    Rename,
    markers,
)
from ._meta import IDENTITY_OF_IDENTITIES
from ._model import encode

_FIELD_MARKERS: Final = (Rename, OmitNone, OmitEmpty, Quantize, Decimals, AsSet)


class Identity:
    """An identity as data (spec 5): which parts of a value are digested,
    and how each is read. It applies to any value: a dict decoded from
    JSON, a dataclass, a pydantic model.

    An identity is built from a type, as pydantic's ``TypeAdapter`` takes
    any type: ``Identity(list[str])``, ``Identity(Task)`` for a declared
    class or a ``TypedDict``. ``Annotated`` markers say what a type cannot:
    ``Annotated[datetime, Unit("ms")]``. It is also read from its JSON form,
    which every implementation of quiddity reads and writes, with
    :meth:`from_json`.

    >>> from typing import NotRequired, TypedDict
    >>> from datetime import datetime
    >>> import quiddity as q
    >>> class Task(TypedDict):
    ...     prompt: str
    ...     priority: int
    ...     asked_at: datetime
    >>> task = q.Identity(Task)
    >>> a = task.digest(
    ...     {"prompt": "p", "priority": 1, "asked_at": "2024-01-01T00:00:00Z", "tag": "x"}
    ... )
    >>> b = task.digest({"prompt": "p", "priority": "1", "asked_at": "2024-01-01T01:00:00+01:00"})
    >>> a == b
    True

    Members the identity does not list, such as ``"tag"``, are left out.
    Values written in other forms are converted: the text ``"1"`` to an
    integer, RFC 3339 text to an instant. A value of the wrong shape is an
    error.

    An identity is immutable, hashable and always valid (spec 5.9): every
    method that changes one returns a new identity, and raises
    :class:`InvalidIdentity` when the result would not be valid. Its own
    digest is ``quiddity.digest(identity)`` (spec 5.10).
    """

    __slots__ = ("_applier", "_defs", "_domain", "_key", "_root")
    __quiddity_domain__: ClassVar[str] = "quiddity.identity"

    _root: Kind
    _defs: Defs
    _domain: str | None
    _key: str
    _applier: Applier | None

    def __init__(self, kind: Any, *, domain: str | None = None) -> None:
        b = Builder()
        root = b.kind(kind)
        if domain is None and isinstance(kind, type) and _declared.sources(kind) is not None:
            domain = _declared.config(kind).domain
        self._set(*_checked(root, b.defs, domain))

    def _set(self, root: Kind, defs: Defs, domain: str | None) -> None:
        object.__setattr__(self, "_root", root)
        object.__setattr__(self, "_defs", defs)
        object.__setattr__(self, "_domain", domain)
        object.__setattr__(self, "_key", _load.canon_key(self.to_json()))
        object.__setattr__(self, "_applier", None)

    @classmethod
    def _make(cls, root: Kind, defs: Defs, domain: str | None) -> Identity:
        identity = object.__new__(cls)
        identity._set(*_checked(root, defs, domain))
        return identity

    # --- JSON -----------------------------------------------------------------------

    @classmethod
    def from_json(cls, source: str | bytes | Mapping[str, Any]) -> Identity:
        """Reads an identity from its JSON form (spec 5.3): JSON text, or a
        dict decoded from it. An object with a repeated member name is an
        error, and so is a number where an integer is required, such as
        ``5.0``.

        >>> Identity.from_json('{"root": {"list": "string"}}') == Identity(list[str])
        True
        >>> Identity.from_json({"root": "text"})
        Traceback (most recent call last):
        ...
        quiddity.InvalidIdentity: unknown kind 'text' (at $.root)
        """
        if isinstance(source, str | bytes | bytearray):
            doc = parse_json(source)
        elif isinstance(source, Mapping):
            doc = dict(source)
        else:
            raise TypeError(
                f"an identity's JSON form is text or a dict, not {type(source).__name__}"
            )
        root, defs, domain = _load.load(doc)
        identity = object.__new__(cls)
        identity._set(root, defs, domain)
        return identity

    def to_json(self) -> dict[str, Any]:
        """The identity's JSON form, in canonical form, as a dict ready for
        ``json.dumps``.

        >>> Identity(list[str]).to_json()
        {'root': {'list': 'string'}}
        """
        out: dict[str, Any] = {"root": copy.deepcopy(self._root)}
        if self._domain is not None:
            out["domain"] = self._domain
        if self._defs:
            out["definitions"] = copy.deepcopy(self._defs)
        return out

    # --- Reading --------------------------------------------------------------------

    @property
    def domain(self) -> str | None:
        """The domain of digests under this identity (spec 4.2), or None."""
        return self._domain

    @property
    def name(self) -> str | None:
        """The name of the identity's own definition, if its root is one: the
        name given with :meth:`named`, or a class's name."""
        r = self._root
        return r["ref"] if isinstance(r, dict) and "ref" in r else None

    def digest(self, value: Any) -> Digest:
        """The digest of ``value`` under this identity (spec 5.1). Declared
        classes in ``value`` are read as their primary identities digest
        them; a value is read as its JSON form would be."""
        d = Digest.of_encoding(encode(self.read(value)))
        return d.in_domain(self._domain) if self._domain is not None else d

    def read(self, value: Any) -> Any:
        """The data-model value of ``value`` under this identity."""
        applier = self._applier
        if applier is None:
            applier = Applier(self._defs)
            object.__setattr__(self, "_applier", applier)
        return applier.read(self._root, _value.from_python(value), "$")

    # --- Changing -------------------------------------------------------------------

    def named(self, name: str) -> Identity:
        """The identity, named ``name``: its root becomes its own definition
        under that name, which is how it appears in its JSON form and in
        those of identities it is nested in. Naming a named identity renames
        its definition and the references to it.

        >>> row = Identity(dict[str, int]).named("Row")
        >>> row.to_json()["root"], row.name
        ({'ref': 'Row'}, 'Row')
        """
        if not isinstance(name, str) or not name:
            raise InvalidIdentity(f"a name is a non-empty string, not {name!r}")
        defs = copy.deepcopy(self._defs)
        old = self.name
        if old == name:
            return self
        if name in defs:
            raise InvalidIdentity(f"the identity already has a definition named {name!r}")
        if old is not None:
            defs[name] = defs.pop(old)
            defs = {n: _rename_refs(k, old, name) for n, k in defs.items()}
        else:
            defs[name] = copy.deepcopy(self._root)
        return Identity._make({"ref": name}, defs, self._domain)

    def with_domain(self, domain: str | None) -> Identity:
        """The identity with another domain, or none."""
        return Identity._make(copy.deepcopy(self._root), copy.deepcopy(self._defs), domain)

    def with_field(self, name: str, kind: Any) -> Identity:
        """The identity with the field ``name`` added to its root object, or
        replaced. ``kind`` is a type, with field markers in ``Annotated``:
        ``Annotated[datetime, OmitNone()]``.

        >>> base = Identity(dict[str, str])
        >>> Identity.from_json({"root": {"object": {"fields": {}}}}).with_field("n", int).to_json()
        {'root': {'object': {'fields': {'n': {'kind': 'integer'}}}}}
        """
        return self._change(None, name, kind)

    def without_field(self, name: str) -> Identity:
        """The identity without the field ``name`` of its root object."""
        return self._change(None, name, _REMOVE)

    def with_field_in(self, definition: str, name: str, kind: Any) -> Identity:
        """As :meth:`with_field`, in the object of the definition named
        ``definition``."""
        return self._change(definition, name, kind)

    def without_field_in(self, definition: str, name: str) -> Identity:
        """As :meth:`without_field`, in the object of the definition named
        ``definition``."""
        return self._change(definition, name, _REMOVE)

    def _change(self, definition: str | None, name: str, kind: Any) -> Identity:
        b = Builder()
        root = b.absorb(self)
        root = copy.deepcopy(root)
        b.defs = copy.deepcopy(b.defs)
        if definition is None:
            target = b.defs.get(root["ref"]) if isinstance(root, dict) and "ref" in root else root
        else:
            if definition not in b.defs:
                raise InvalidIdentity(f"the identity has no definition named {definition!r}")
            target = b.defs[definition]
        if not (isinstance(target, dict) and "object" in target):
            where = f"the definition {definition!r}" if definition else "the identity's root"
            raise InvalidIdentity(f"{where} is not an object")
        fields = target["object"]["fields"]
        if kind is _REMOVE:
            if name not in fields:
                raise InvalidIdentity(f"there is no field named {name!r}")
            del fields[name]
            order = target["object"].get("order")
            if order is not None:
                order.remove(name)
        else:
            fields[name] = _field(b, kind)
            order = target["object"].get("order")
            if order is not None and name not in order:
                order.append(name)
        return Identity._make(root, b.defs, self._domain)

    # --- Values -----------------------------------------------------------------------

    def __quiddity__(self, identity: str) -> Any:
        if identity != _declared.PRIMARY:
            raise QuiddityError(f"an Identity has no identity {identity!r}")
        meta = _meta()
        return meta.read(self.to_json())

    def __eq__(self, other: object) -> bool:
        return isinstance(other, Identity) and other._key == self._key

    def __hash__(self) -> int:
        return hash(self._key)

    def __repr__(self) -> str:
        return f"Identity.from_json({self.to_json()!r})"

    def __setattr__(self, name: str, value: object) -> None:
        raise AttributeError("an Identity is immutable")

    def __reduce__(self) -> tuple[Any, tuple[dict[str, Any]]]:
        return (Identity.from_json, (self.to_json(),))


_REMOVE: Final = object()


def _field(b: Builder, kind: Any) -> dict[str, Any]:
    tp, metadata = _declared.split_annotated(kind)
    fmarkers, kmarkers = markers(metadata)
    for m in fmarkers:
        if not isinstance(m, _FIELD_MARKERS):
            raise InvalidIdentity(f"{m!r} does not apply to a field of an identity")
        if m.identities:
            raise InvalidIdentity(
                f"{m!r} names identities, which a field of an identity does not have"
            )
    by_type: dict[type[FieldMarker], FieldMarker] = {}
    for m in fmarkers:
        if type(m) in by_type:
            raise InvalidIdentity(f"{by_type[type(m)]!r} and {m!r} both apply")
        by_type[type(m)] = m
    k = b.kind(tp, kind_markers=tuple(kmarkers))
    if AsSet in by_type:
        from ._kinds import _as_set

        k = _as_set(k)
    field: dict[str, Any] = {}
    omit = []
    if OmitNone in by_type or OmitEmpty in by_type:
        omit.append("none")
        k = _optional(k)
    if OmitEmpty in by_type:
        omit.append("empty")
    field["kind"] = k
    if omit:
        field["omit"] = omit
    r = by_type.get(Rename)
    if isinstance(r, Rename):
        field["rename"] = r.key
    q = by_type.get(Quantize)
    if isinstance(q, Quantize):
        field["quantize"] = q.step
    d = by_type.get(Decimals)
    if isinstance(d, Decimals):
        field["decimals"] = d.places
    return field


def _rename_refs(k: Kind, old: str, new: str) -> Kind:
    if isinstance(k, dict):
        if k.get("ref") == old and len(k) == 1:
            return {"ref": new}
        return {key: _rename_refs(v, old, new) for key, v in k.items()}
    if isinstance(k, list):
        return [_rename_refs(v, old, new) for v in k]
    return k


def _checked(root: Kind, defs: Defs, domain: str | None) -> tuple[Kind, Defs, str | None]:
    """Parts built in code, in canonical form and checked."""
    canon = {n: _load.canonical(k, f"$.definitions.{n}") for n, k in defs.items()}
    return _load.build(_load.canonical(root, "$.root"), canon, domain)


def parse_json(text: str | bytes | bytearray) -> Any:
    """JSON text, rejecting repeated member names and ``NaN``."""

    def pairs(items: list[tuple[str, Any]]) -> dict[str, Any]:
        out: dict[str, Any] = {}
        for k, v in items:
            if k in out:
                raise InvalidIdentity(f"the member name {k!r} is repeated")
            out[k] = v
        return out

    def constant(name: str) -> Any:
        raise InvalidIdentity(f"{name} is not JSON")

    try:
        return json.loads(text, object_pairs_hook=pairs, parse_constant=constant)
    except ValueError as e:
        if isinstance(e, InvalidIdentity):
            raise
        raise InvalidIdentity(f"not JSON: {e}") from None


_META: list[Identity] = []


def _meta() -> Identity:
    """The identity of identities (spec Appendix C)."""
    if not _META:
        _META.append(Identity.from_json(IDENTITY_OF_IDENTITIES))
    return _META[0]


def identity_of(cls: type, identity: str = _declared.PRIMARY) -> Identity:
    """The identity of a declared class as data: the identity, in its JSON
    form, that another language can apply to the class's JSON form, as
    Rust's ``construct_identity()``.

    A field's kind is the one its type gives it (PY7): ``str`` is a string,
    ``datetime`` an instant, ``T | None`` optional, another declared class
    its identity, nested. A type quiddity cannot describe is ``any``.

    >>> from dataclasses import dataclass
    >>> import quiddity as q
    >>> @q.digestible(domain="example.point")
    ... @dataclass
    ... class Point:
    ...     x: int
    ...     y: int
    >>> q.identity_of(Point).to_json()["definitions"]["Point"]
    {'object': {'fields': {'x': {'kind': 'integer'}, 'y': {'kind': 'integer'}}}}
    """
    if not isinstance(cls, type) or _declared.sources(cls) is None:
        raise TypeError(f"{cls!r} is not a declared class")
    b = Builder()
    root = b.declared(cls, identity)
    return Identity._make(root, b.defs, _declared.config(cls).domain)
