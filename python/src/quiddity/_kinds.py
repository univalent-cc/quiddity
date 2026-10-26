"""Python types as kinds (``history/PYTHON.md``, PY7): for identities written
as types, and for the identities declared classes export."""

from __future__ import annotations

import collections.abc as cabc
import datetime as dt
import enum
import types
import typing
import uuid
from decimal import Decimal
from pathlib import PurePath
from typing import TYPE_CHECKING, Any, Final, Literal, Union, get_args, get_origin

from . import _declared, _load
from ._digest import Digest
from ._errors import InvalidIdentity, QuiddityError
from ._load import Defs, Kind
from ._markers import Hex, KindMarker, Tagged, Unit

if TYPE_CHECKING:
    from ._identity import Identity

_SCALARS: Final[dict[Any, Kind]] = {
    bool: "bool",
    int: "integer",
    float: "number",
    str: "string",
    uuid.UUID: "uuid",
    dt.date: "string",
    Decimal: "string",
    type(None): "null",
    None: "null",
    Any: "any",
    object: "any",
}
_LISTS: Final = {list, cabc.Sequence, cabc.MutableSequence, cabc.Iterable, cabc.Collection}
_SETS: Final = {set, frozenset, cabc.Set, cabc.MutableSet}
_MAPS: Final = {dict, cabc.Mapping, cabc.MutableMapping}
_PLACEHOLDER: Final[Kind] = {"object": {"fields": {}}}


class Builder:
    """Builds the kinds of one identity, collecting its definitions."""

    def __init__(self) -> None:
        self.defs: Defs = {}
        self._names: dict[Any, str] = {}  # (class, identity) or Identity -> definition name
        self._taken: dict[str, Any] = {}  # definition name -> what it names
        self._building: set[str] = set()

    # --- Definitions ------------------------------------------------------------

    def _name(self, key: Any, cls: type, identity: str = _declared.PRIMARY) -> tuple[str, bool]:
        """The definition name for ``key``, and whether it is new. Types with
        the same name are told apart by their module paths, as in Rust."""
        if key in self._names:
            return self._names[key], False
        suffix = "" if identity == _declared.PRIMARY else f"/{identity}"
        name = cls.__name__ + suffix
        if name in self._taken:
            name = f"{cls.__module__}.{cls.__qualname__}{suffix}"
        self._names[key] = name
        self._taken[name] = key
        return name, True

    def absorb(self, identity: Identity) -> Kind:
        """An identity nested in this one: its definitions join this one's,
        and its root is returned."""
        for name, k in identity._defs.items():
            existing = self.defs.get(name)
            if existing is not None and _load.canon_key(existing) != _load.canon_key(k):
                raise InvalidIdentity(f"two different definitions are named {name!r}")
            self.defs[name] = k
            self._taken.setdefault(name, identity)
        return identity._root

    # --- Types ------------------------------------------------------------------

    def kind(
        self,
        tp: Any,
        identity: str = _declared.PRIMARY,
        kind_markers: tuple[KindMarker, ...] = (),
    ) -> Kind:
        """The kind of type ``tp``. ``identity`` is the identity a declared
        class at its top level is read with; containers read their items
        with their primary identities."""
        from ._identity import Identity

        tp, metadata = _declared.split_annotated(tp)
        kind_markers = (*kind_markers, *(m for m in metadata if isinstance(m, KindMarker)))
        # pydantic's Field(discriminator=...) and Discriminator(...).
        kind_markers += tuple(
            Tagged(d)
            for m in metadata
            if not isinstance(m, KindMarker)
            and isinstance(d := getattr(m, "discriminator", None), str)
        )
        if isinstance(tp, Identity):
            return self.absorb(tp)
        if isinstance(tp, str | typing.ForwardRef):
            raise InvalidIdentity(f"the type {tp!r} is not resolved; pass the type itself")
        origin = get_origin(tp)
        args = get_args(tp)

        if origin is Union or origin is types.UnionType:
            return self._union(args, identity, kind_markers)
        if origin is Literal:
            return self._values(args)
        if isinstance(tp, typing.TypeAliasType if hasattr(typing, "TypeAliasType") else ()):
            return self.kind(tp.__value__, identity, kind_markers)
        if isinstance(tp, typing.NewType):
            return self.kind(tp.__supertype__, identity, kind_markers)
        if tp in _SCALARS:
            return _SCALARS[tp]
        if tp is Digest:
            return "string"
        if tp in (bytes, bytearray, memoryview):
            return {
                "bytes": {"text": "hex"} if any(isinstance(m, Hex) for m in kind_markers) else {}
            }
        if tp is dt.datetime or tp is dt.timedelta or tp is dt.time:
            name = "instant" if tp is dt.datetime else "duration"
            units = [m.unit for m in kind_markers if isinstance(m, Unit)]
            return {name: {"unit": units[-1]} if units else {}}
        if isinstance(tp, type) and issubclass(tp, PurePath):
            return "path"
        if origin in _LISTS or tp in _LISTS:
            return {"list": self.kind(args[0]) if args else "any"}
        if origin is tuple or tp is tuple:
            if not args or (len(args) == 2 and args[1] is Ellipsis):
                return {"list": self.kind(args[0]) if args else "any"}
            if args == ((),):
                return "any"
            return {"tuple": [self.kind(a) for a in args]}
        if origin in _SETS or tp in _SETS:
            return {"set": self.kind(args[0]) if args else "any"}
        if origin in _MAPS or tp in _MAPS:
            key, value = args if args else (str, Any)
            return {"map": {"key": self.kind(key), "value": self.kind(value)}}
        if isinstance(tp, type):
            if issubclass(tp, enum.Enum):
                return self._values(tuple(m.value for m in tp))
            if issubclass(tp, tuple) and hasattr(tp, "_fields"):
                # A NamedTuple is a tuple, as json and pydantic write it.
                hints = typing.get_type_hints(tp)
                kinds = [self.kind(hints.get(f, Any)) for f in tp._fields]
                return {"tuple": kinds} if kinds else "any"
            if _declared.sources(tp) is not None:
                return self.declared(tp, identity)
            root = _pydantic_root(tp)
            if root is not None:
                return self.kind(root, identity, kind_markers)
            for base, k in _SCALARS.items():
                if isinstance(base, type) and base is not object and issubclass(tp, base):
                    if base is dt.date and issubclass(tp, dt.datetime):
                        continue
                    return k
            if issubclass(tp, dt.datetime):
                return self.kind(dt.datetime, identity, kind_markers)
            if issubclass(tp, dt.time):
                return self.kind(dt.time, identity, kind_markers)
        return "any"

    def declared(self, cls: type, identity: str = _declared.PRIMARY) -> Kind:
        """A declared class (or TypedDict) under ``identity``, as a
        definition named after it."""
        name, new = self._name((cls, identity), cls, identity)
        if new:
            self._building.add(name)
            try:
                self.defs[name] = {"object": {"fields": self._fields(cls, identity)}}
            finally:
                self._building.discard(name)
        return {"ref": name}

    def _fields(self, cls: type, identity: str) -> dict[str, Any]:
        try:
            plan = _declared.plan(cls, identity)
        except QuiddityError as e:
            raise InvalidIdentity(e.message) from None
        fields: dict[str, Any] = {}
        for f in plan.fields:
            if f.flatten:
                inner = self.kind(f.hint, f.nested or _declared.PRIMARY)
                inner = (
                    self.defs.get(inner["ref"])
                    if isinstance(inner, dict) and "ref" in inner
                    else None
                )
                if not (
                    isinstance(inner, dict)
                    and "object" in inner
                    and "order" not in inner["object"]
                ):
                    raise InvalidIdentity(
                        f"{cls.__qualname__}.{f.attr}: only a declared class can be flattened in "
                        "an identity"
                    )
                for member, field in inner["object"]["fields"].items():
                    if member in fields:
                        raise InvalidIdentity(
                            f"{cls.__qualname__}: two fields are named {member!r}"
                        )
                    fields[member] = field
                continue
            if f.member in fields:
                raise InvalidIdentity(f"{cls.__qualname__}: two fields are named {f.member!r}")
            fields[f.member] = self._field(f)
        return fields

    def _field(self, f: _declared.FieldPlan) -> dict[str, Any]:
        k = self.kind(f.hint, f.nested or _declared.PRIMARY, f.kind_markers)
        if f.as_set:
            k = _as_set(k)
        omit = []
        if f.omit_none or f.omit_empty or f.optional:
            omit.append("none")
            k = _optional(k)
        if f.omit_empty:
            omit.append("empty")
        field: dict[str, Any] = {"kind": k}
        if f.key != f.member:
            field["rename"] = f.key
        if omit:
            field["omit"] = omit
        if f.quantize is not None:
            field["quantize"] = f.quantize
        if f.decimals is not None:
            field["decimals"] = f.decimals
        return field

    def _union(
        self, args: tuple[Any, ...], identity: str, kind_markers: tuple[KindMarker, ...]
    ) -> Kind:
        tagged = [m for m in kind_markers if isinstance(m, Tagged)]
        if tagged:
            return self._tagged(args, tagged[-1])
        alts = [self.kind(a, identity, kind_markers) for a in args]
        return self._valid_union(alts)

    def _valid_union(self, alts: list[Kind]) -> Kind:
        """A union of ``alts``, if it is a valid one (spec 5.7); else ``any``,
        as a type that quiddity cannot tell apart is read as it is."""
        try:
            union = _load.canonical({"union": alts}, "$")
            defs = {n: self.defs.get(n, _PLACEHOLDER) for n in self._taken}
            defs.update({n: _PLACEHOLDER for n in self._building})
            for n in _refs(union):
                defs.setdefault(n, _PLACEHOLDER)
            _load.claims(union, defs, "$")
            return union
        except InvalidIdentity:
            return "any"

    def _values(self, values: tuple[Any, ...]) -> Kind:
        """The kind of a ``Literal``'s or an ``Enum``'s values."""
        kinds = []
        for v in values:
            if isinstance(v, enum.Enum):
                v = v.value
            if v is None:
                kinds.append("null")
            elif type(v) is bool:
                kinds.append("bool")
            elif type(v) is int:
                kinds.append("integer")
            elif type(v) is float:
                kinds.append("number")
            elif type(v) is str:
                kinds.append("string")
            else:
                return "any"
        if not kinds:
            return "any"
        return self._valid_union(kinds)

    def _tagged(self, args: tuple[Any, ...], marker: Tagged) -> Kind:
        """An internally tagged union of declared classes (spec 5.8): each
        class names its variant with a ``Literal`` field named like the tag,
        as pydantic's discriminated unions do."""
        tag = marker.tag
        nullable = type(None) in args
        variants: dict[str, Any] = {}
        for cls in (a for a in args if a is not type(None)):
            cls = _declared.split_annotated(cls)[0]
            if not isinstance(cls, type) or _declared.sources(cls) is None:
                raise InvalidIdentity(
                    f"Tagged applies to a union of declared classes, not {cls!r}"
                )
            ref = self.declared(cls)
            obj = self.defs[ref["ref"]]["object"]
            plan = _declared.plan(cls, _declared.PRIMARY)
            labels = [
                get_args(f.hint)[0]
                for f in plan.fields
                if f.member == tag and get_origin(f.hint) is Literal and len(get_args(f.hint)) == 1
            ]
            if not labels or not isinstance(labels[0], str):
                raise InvalidIdentity(
                    f"{cls.__qualname__} has no field {tag!r} of a Literal string type, which "
                    "names its variant"
                )
            fields = {k: v for k, v in obj["fields"].items() if k != tag}
            # A class with only the tag is a unit variant.
            variants[labels[0]] = {"kind": {"object": {"fields": fields}}} if fields else {}
        kind = {"tagged": {"style": "internal", "tag": tag, "variants": variants}}
        return {"union": ["null", kind]} if nullable else kind


def _pydantic_root(tp: type) -> Any:
    """The type of a pydantic ``RootModel``'s root, or None."""
    import sys

    pydantic = sys.modules.get("pydantic")
    if pydantic is not None and issubclass(tp, pydantic.RootModel):
        return typing.cast(Any, tp).model_fields["root"].annotation
    return None


def _refs(k: Kind) -> set[str]:
    found: set[str] = set()
    _load.reach(k, {}, found)
    return found


def _optional(k: Kind) -> Kind:
    if k == "any" or k == "null":
        return k
    if isinstance(k, dict) and "union" in k:
        return k if "null" in k["union"] else {"union": ["null", *k["union"]]}
    return {"union": ["null", k]}


def _as_set(k: Kind) -> Kind:
    if isinstance(k, dict) and "list" in k:
        return {"set": k["list"]}
    if isinstance(k, dict) and "union" in k:
        return {"union": [_as_set(a) for a in k["union"]]}
    if k == "any":
        return {"set": "any"}
    return k
