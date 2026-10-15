"""Declared classes: dataclasses, attrs classes and pydantic models, digested
field by field, with options in ``Annotated`` markers and the
:func:`digestible` decorator (``history/PYTHON.md``, PY5)."""

from __future__ import annotations

import contextlib
import dataclasses
import sys
import typing
import weakref
from collections.abc import Callable, Sequence
from typing import Annotated, Any, Final, Literal, TypeVar, get_args, get_origin

from . import _value
from ._errors import QuiddityError
from ._markers import (
    AsSet,
    Decimals,
    FieldMarker,
    Flatten,
    KindMarker,
    Nested,
    OmitEmpty,
    OmitNone,
    Quantize,
    Rename,
    Skip,
    markers,
)
from ._model import Map, Set, is_empty
from ._quantize import decimals, quantize

PRIMARY: Final = "primary"

RenameRule = Literal[
    "camelCase",
    "PascalCase",
    "snake_case",
    "kebab-case",
    "SCREAMING_SNAKE_CASE",
    "SCREAMING-KEBAB-CASE",
    "lowercase",
    "UPPERCASE",
]
_RULES: Final = typing.get_args(RenameRule)


class _NotDeclared:
    def __repr__(self) -> str:
        return "NOT_DECLARED"


NOT_DECLARED: Final = _NotDeclared()


@dataclasses.dataclass(frozen=True)
class Config:
    """What :func:`digestible` declares for a class as a whole."""

    domain: str | None = None
    identities: tuple[str, ...] = ()
    rename_all: str | None = None


@dataclasses.dataclass(frozen=True)
class Source:
    """A field as its class declares it, before markers are applied."""

    attr: str
    key: str  # the field's key before renaming: its name, or pydantic's alias
    hint: Any  # its type, without Annotated markers
    metadata: tuple[Any, ...]  # its Annotated metadata
    serializer: Callable[[Any, Any], Any] | None = None  # pydantic's field_serializer
    renamed: bool = False  # whether `key` is an alias, which rename_all leaves alone
    optional: bool = False  # whether the member may be missing (a TypedDict's NotRequired)


@dataclasses.dataclass(frozen=True)
class FieldPlan:
    """How a field is digested under one identity."""

    attr: str
    member: str  # the field's member in the class's JSON form
    key: str  # the field's key in the digest
    hint: Any
    omit_none: bool
    omit_empty: bool
    quantize: int | None
    decimals: int | None
    as_set: bool
    flatten: bool
    nested: str | None
    serializer: Callable[[Any, Any], Any] | None
    kind_markers: tuple[KindMarker, ...]
    optional: bool = False


@dataclasses.dataclass(frozen=True)
class Plan:
    """How a class is digested under one identity."""

    cls: type
    identity: str
    domain: str | None
    fields: tuple[FieldPlan, ...]


_configs: weakref.WeakKeyDictionary[type, Config] = weakref.WeakKeyDictionary()
_plans: weakref.WeakKeyDictionary[type, dict[str, Plan]] = weakref.WeakKeyDictionary()
_sources: weakref.WeakKeyDictionary[type, tuple[Source, ...] | None] = weakref.WeakKeyDictionary()
# The names visible where a decorated class was defined, for its type hints
# (as pydantic keeps a model's parent namespace).
_namespaces: weakref.WeakKeyDictionary[type, dict[str, Any]] = weakref.WeakKeyDictionary()

C = TypeVar("C", bound=type)


@typing.overload
def digestible(cls: C, /) -> C: ...


@typing.overload
def digestible(
    *,
    domain: str | None = None,
    identities: Sequence[str] = (),
    rename_all: RenameRule | None = None,
) -> Callable[[C], C]: ...


def digestible(
    cls: C | None = None,
    /,
    *,
    domain: str | None = None,
    identities: Sequence[str] = (),
    rename_all: RenameRule | None = None,
) -> C | Callable[[C], C]:
    """Declares how a class is digested as a whole.

    The class is a dataclass, an attrs class, a pydantic model or a
    ``TypedDict``; put this decorator above ``@dataclass``. Its fields are digested under their
    names, with the options of their ``Annotated`` markers. A dataclass
    without this decorator is digested the same way, with the defaults.

    - ``domain``: a label hashed into every digest of the class, so that
      its digests differ from those of other types with the same fields
      (spec 4.2).
    - ``identities``: the names of the class's identities besides
      ``"primary"``, which markers can be scoped to and which
      :func:`quiddity.digest` takes.
    - ``rename_all``: a case convention for every field's key, as serde
      names them: ``"camelCase"``, ``"PascalCase"``, ``"snake_case"``,
      ``"kebab-case"``, ``"SCREAMING_SNAKE_CASE"``,
      ``"SCREAMING-KEBAB-CASE"``, ``"lowercase"`` or ``"UPPERCASE"``. A
      field's ``Rename`` takes precedence.
    """
    if domain is not None and (not isinstance(domain, str) or not domain):
        raise TypeError(f"a domain is a non-empty string, not {domain!r}")
    if isinstance(identities, str):
        raise TypeError("identities is a list of names, not a string")
    names = tuple(identities)
    for name in names:
        if not isinstance(name, str) or not name:
            raise TypeError(f"an identity name is a non-empty string, not {name!r}")
    if PRIMARY in names:
        raise TypeError('"primary" is always an identity; list only the others')
    if len(set(names)) != len(names):
        raise TypeError(f"an identity is listed twice in {names!r}")
    if rename_all is not None and rename_all not in _RULES:
        raise TypeError(f"rename_all is one of {', '.join(_RULES)}, not {rename_all!r}")
    config = Config(domain, names, rename_all)
    frame = sys._getframe(1)
    namespace = dict(frame.f_locals) if frame.f_locals is not frame.f_globals else {}
    del frame

    def decorate(c: C) -> C:
        if not declarable(c):
            raise TypeError(
                f"@digestible applies to a dataclass, an attrs class, a pydantic model or a "
                f"TypedDict, not {c!r}; put it above @dataclass"
            )
        _configs[c] = config
        _namespaces[c] = {**namespace, c.__name__: c}
        _plans.pop(c, None)
        _sources.pop(c, None)
        return c

    return decorate if cls is None else decorate(cls)


def config(cls: type) -> Config:
    for c in cls.__mro__:
        found = _configs.get(c)
        if found is not None:
            return found
    return Config()


def identities(cls: type) -> tuple[str, ...]:
    """The names of a declared class's identities, ``"primary"`` first."""
    return (PRIMARY, *config(cls).identities)


# --- Reading a class's fields -------------------------------------------------


def declarable(cls: Any) -> bool:
    """Whether ``cls`` is a class whose fields can be declared, without
    reading its type hints, which may name classes not yet defined."""
    if not isinstance(cls, type):
        return False
    pydantic = sys.modules.get("pydantic")
    if pydantic is not None and issubclass(cls, pydantic.BaseModel):
        from . import _pydantic

        return not _pydantic.has_model_serializer(cls)
    return (
        dataclasses.is_dataclass(cls)
        or hasattr(cls, "__attrs_attrs__")
        or typing.is_typeddict(cls)
    )


def sources(cls: type) -> tuple[Source, ...] | None:
    """The fields of a declared class, or None if ``cls`` is not one."""
    try:
        return _sources[cls]
    except KeyError:
        pass
    except TypeError:
        return None
    found = _read_sources(cls)
    with contextlib.suppress(TypeError):
        _sources[cls] = found
    return found


def _read_sources(cls: type) -> tuple[Source, ...] | None:
    pydantic = sys.modules.get("pydantic")
    if pydantic is not None and isinstance(cls, type) and issubclass(cls, pydantic.BaseModel):
        from . import _pydantic

        return None if _pydantic.has_model_serializer(cls) else _pydantic.sources(cls)
    if dataclasses.is_dataclass(cls) and isinstance(cls, type):
        names = [f.name for f in dataclasses.fields(cls)]
        declared = {f.name: f.type for f in dataclasses.fields(cls)}
        return _from_hints(cls, names, declared)
    attrs_fields = getattr(cls, "__attrs_attrs__", None)
    if attrs_fields is not None and isinstance(cls, type):
        names = [a.name for a in attrs_fields]
        declared = {a.name: a.type for a in attrs_fields}
        return _from_hints(cls, names, declared)
    if typing.is_typeddict(cls):
        # A TypedDict's values are dicts; its fields serve as a kind (PY6).
        td: Any = cls
        names = sorted(td.__required_keys__ | td.__optional_keys__)
        try:
            declared = dict(td.__annotations__)
        except NameError:
            declared = {}
        return _from_hints(cls, names, declared, optional=td.__optional_keys__)
    return None


_REQUIRED_WRAPPERS: Final = {typing.Required, typing.NotRequired} | (
    {typing.ReadOnly} if hasattr(typing, "ReadOnly") else set()
)


def _from_hints(
    cls: type, names: list[str], declared: dict[str, Any], optional: frozenset[str] = frozenset()
) -> tuple[Source, ...]:
    try:
        hints = typing.get_type_hints(cls, localns=_namespaces.get(cls), include_extras=True)
    except NameError:
        # A forward reference to a local class: types given as objects
        # still serve.
        hints = {}
    out = []
    for name in names:
        hint = hints.get(name, declared.get(name, Any))
        if hint is None:
            hint = Any
        if isinstance(hint, (str, typing.ForwardRef)):
            raise TypeError(
                f"the type of {cls.__qualname__}.{name}, {hint!r}, cannot be resolved: a "
                "class defined in a function is visible to quiddity only if decorated with "
                "@digestible, or if its module does not use `from __future__ import annotations`"
            )
        wrapper = _wrapper(hint)
        is_optional = wrapper is typing.NotRequired or (
            name in optional and wrapper is not typing.Required
        )
        base, metadata = split_annotated(hint)
        out.append(Source(name, name, base, metadata, optional=is_optional))
    return tuple(out)


def _wrapper(hint: Any) -> Any:
    """``Required`` or ``NotRequired``, if a TypedDict field's type is
    wrapped in one, inside ``Annotated`` or not."""
    while True:
        origin = get_origin(hint)
        if origin is typing.Required or origin is typing.NotRequired:
            return origin
        if origin is Annotated or (origin is not None and origin in _REQUIRED_WRAPPERS):
            hint = get_args(hint)[0]
        else:
            return None


def split_annotated(hint: Any) -> tuple[Any, tuple[Any, ...]]:
    """A type without its outer ``Annotated``, and that metadata."""
    metadata: tuple[Any, ...] = ()
    while True:
        origin = get_origin(hint)
        if origin is Annotated:
            args = get_args(hint)
            hint, metadata = args[0], (*args[1:], *metadata)
        elif origin is not None and origin in _REQUIRED_WRAPPERS:
            hint = get_args(hint)[0]
        else:
            return hint, metadata


# --- Plans ----------------------------------------------------------------------


def plan(cls: type, identity: str) -> Plan:
    """How ``cls`` is digested under ``identity``, raising ``QuiddityError``
    if it has no such identity."""
    by_identity = _plans.get(cls)
    if by_identity is None:
        by_identity = {}
        _plans[cls] = by_identity
    found = by_identity.get(identity)
    if found is None:
        found = _build(cls, identity)
        by_identity[identity] = found
    return found


def _build(cls: type, identity: str) -> Plan:
    srcs = sources(cls)
    assert srcs is not None
    conf = config(cls)
    names = (PRIMARY, *conf.identities)
    if identity not in names:
        raise QuiddityError(f"{cls.__qualname__} has no identity {identity!r}")
    fields = []
    for src in srcs:
        fmarkers, kmarkers = markers(src.metadata)
        for m in fmarkers:
            for scope in m.identities:
                if scope not in names:
                    raise TypeError(
                        f"{cls.__qualname__}.{src.attr}: {m!r} names the identity {scope!r}, "
                        f"which the class does not declare (with @digestible(identities=...))"
                    )
        chosen = {kind: _choose(fmarkers, kind, identity, cls, src) for kind in _OPTIONS}
        if chosen[Skip] is not None:
            continue
        rename = chosen[Rename]
        if isinstance(rename, Rename):
            key = rename.key
        elif src.renamed:
            key = src.key
        else:
            key = apply_rule(src.key, conf.rename_all)
        q, d = chosen[Quantize], chosen[Decimals]
        n = chosen[Nested]
        f = FieldPlan(
            attr=src.attr,
            member=src.key,
            key=key,
            hint=src.hint,
            omit_none=chosen[OmitNone] is not None,
            omit_empty=chosen[OmitEmpty] is not None,
            quantize=q.step if isinstance(q, Quantize) else None,
            decimals=d.places if isinstance(d, Decimals) else None,
            as_set=chosen[AsSet] is not None,
            flatten=chosen[Flatten] is not None,
            nested=n.identity if isinstance(n, Nested) else None,
            serializer=src.serializer,
            kind_markers=tuple(kmarkers),
            optional=src.optional,
        )
        where = f"{cls.__qualname__}.{src.attr}"
        if f.quantize is not None and f.decimals is not None:
            raise TypeError(f"{where} has Quantize or Decimals, not both")
        if f.flatten and (
            isinstance(rename, Rename)
            or f.omit_none
            or f.omit_empty
            or f.quantize is not None
            or f.decimals is not None
            or f.as_set
        ):
            raise TypeError(
                f"{where}: Flatten cannot be combined with options for its key or value"
            )
        fields.append(f)
    keys = [f.key for f in fields if not f.flatten]
    for k in keys:
        if keys.count(k) > 1:
            raise TypeError(f"{cls.__qualname__}: two fields have the key {k!r}")
    return Plan(cls, identity, conf.domain, tuple(fields))


_OPTIONS: Final = (Skip, Rename, OmitNone, OmitEmpty, Quantize, Decimals, AsSet, Flatten, Nested)


def _choose(
    ms: list[FieldMarker], kind: type[FieldMarker], identity: str, cls: type, src: Source
) -> FieldMarker | None:
    """The marker of type ``kind`` that applies under ``identity``: one
    scoped to it, else an unscoped one."""
    scoped = [m for m in ms if type(m) is kind and m.applies_to(identity)]
    unscoped = [m for m in ms if type(m) is kind and m.applies_to(identity) is None]
    for group in (scoped, unscoped):
        if len(group) > 1:
            raise TypeError(
                f"{cls.__qualname__}.{src.attr}: {group[0]!r} and {group[1]!r} both apply to "
                f"the identity {identity!r}"
            )
        if group:
            return group[0]
    return None


def apply_rule(name: str, rule: str | None) -> str:
    """A snake_case field name in another case convention, as serde's
    ``rename_all`` does."""
    if rule is None or rule in ("snake_case", "lowercase"):
        return name
    if rule == "UPPERCASE" or rule == "SCREAMING_SNAKE_CASE":
        return name.upper()
    if rule == "kebab-case":
        return name.replace("_", "-")
    if rule == "SCREAMING-KEBAB-CASE":
        return name.upper().replace("_", "-")
    pascal = "".join(part[:1].upper() + part[1:] for part in name.split("_"))
    if rule == "PascalCase":
        return pascal
    return pascal[:1].lower() + pascal[1:]


# --- Digesting ------------------------------------------------------------------


def read(x: Any, identity: str, path: str) -> Any:
    """The data-model value of ``x`` under ``identity`` if it is a declared
    class or implements ``__quiddity__``; else ``NOT_DECLARED``."""
    cls = type(x)
    hand = getattr(cls, "__quiddity__", None)
    if hand is not None:
        return _value.from_python(x.__quiddity__(identity), path)
    pydantic = sys.modules.get("pydantic")
    if pydantic is not None and isinstance(x, pydantic.BaseModel):
        from . import _pydantic

        if _pydantic.has_model_serializer(cls):
            if identity != PRIMARY:
                raise QuiddityError(f"{cls.__qualname__} has no identity {identity!r}", path)
            return _value.from_python(_pydantic.serialized(x), path)
    if sources(cls) is None or typing.is_typeddict(cls):
        return NOT_DECLARED
    return encode(x, plan(cls, identity), path)


def domain(x: Any) -> str | None:
    """The domain of a declared value's digests, or of a class implementing
    ``__quiddity__`` that sets ``__quiddity_domain__``."""
    cls = type(x)
    if getattr(cls, "__quiddity__", None) is not None:
        d = getattr(cls, "__quiddity_domain__", None)
        return d if isinstance(d, str) else None
    if sources(cls) is None:
        return None
    return config(cls).domain


def encode(x: Any, p: Plan, path: str) -> Map:
    entries: list[tuple[Any, Any]] = []
    flattened = False
    for f in p.fields:
        fpath = f"{path}.{f.key}"
        raw = getattr(x, f.attr)
        if f.serializer is not None:
            raw = f.serializer(x, raw)
        if f.nested is not None and raw is not None:
            v = read(raw, f.nested, fpath)
            if v is NOT_DECLARED:
                raise QuiddityError(
                    f"Nested({f.nested!r}) applies to a declared class, not a "
                    f"{type(raw).__qualname__}",
                    fpath,
                )
        else:
            v = _value.from_python(raw, fpath)
        if f.as_set:
            if type(v) is list:
                v = Set(v)
            elif v is not None:
                raise QuiddityError(
                    f"AsSet applies to a list or tuple, not {type(raw).__name__}", fpath
                )
        try:
            if f.quantize is not None:
                v = quantize(v, f.quantize)
            elif f.decimals is not None:
                v = decimals(v, f.decimals)
        except QuiddityError as e:
            raise QuiddityError(e.message, fpath) from None
        if f.flatten:
            if v is None:
                continue
            if type(v) is not Map:
                raise QuiddityError("a flattened field is a mapping or a declared class", fpath)
            entries.extend(v.entries)
            flattened = True
            continue
        if ((f.omit_none or f.omit_empty) and v is None) or (f.omit_empty and is_empty(v)):
            continue
        entries.append((f.key, v))
    if flattened:
        try:
            _value.check_keys(entries, path)
        except QuiddityError as e:
            raise QuiddityError(
                f"{e.message}: a flattened field has a member with the same name as another field",
                path,
            ) from None
    return Map(entries)
