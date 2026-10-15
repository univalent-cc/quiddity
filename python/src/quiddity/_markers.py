"""Markers: options for a field, written in its ``Annotated`` type.

Field markers say how a field of a declared class is digested. Each takes
optional identity names, to apply only under those identities, as Rust's
scoped attributes do: ``Skip()`` skips the field under every identity,
``Skip("meaning")`` under ``meaning`` only. Under an identity, a marker
scoped to it takes precedence over an unscoped marker of the same type.

Kind markers say what a type cannot, where a type stands for a kind
(``Identity(Annotated[datetime, Unit("ms")])``).
"""

from __future__ import annotations

from typing import Any, ClassVar, Literal


class FieldMarker:
    """A field option, possibly scoped to identities."""

    __slots__ = ("identities",)

    identities: tuple[str, ...]
    _options: ClassVar[tuple[str, ...]] = ()

    def __init__(self, *identities: str) -> None:
        for name in identities:
            if not isinstance(name, str) or not name:
                raise TypeError(f"an identity name is a non-empty string, not {name!r}")
        self.identities = identities

    def applies_to(self, identity: str) -> bool | None:
        """True if scoped to ``identity``, None if unscoped, else False."""
        if not self.identities:
            return None
        return identity in self.identities

    def _args(self) -> list[str]:
        return [repr(getattr(self, o)) for o in self._options] + [repr(i) for i in self.identities]

    def __repr__(self) -> str:
        return f"{type(self).__name__}({', '.join(self._args())})"

    def __eq__(self, other: object) -> bool:
        return type(other) is type(self) and repr(other) == repr(self)

    def __hash__(self) -> int:
        return hash(repr(self))


class Skip(FieldMarker):
    """Leave the field out."""

    __slots__ = ()


class Rename(FieldMarker):
    """Digest the field under another key."""

    __slots__ = ("key",)
    _options = ("key",)

    def __init__(self, key: str, *identities: str) -> None:
        if not isinstance(key, str) or not key:
            raise TypeError(f"Rename takes a non-empty string, not {key!r}")
        super().__init__(*identities)
        self.key = key


class OmitNone(FieldMarker):
    """Leave the field out when it is None, so that adding an optional field
    does not change the digests of existing values."""

    __slots__ = ()


class OmitEmpty(FieldMarker):
    """Leave the field out when it is an empty string, list, set or map, or
    None: empty or missing, as Rust's ``Field::omit_empty``."""

    __slots__ = ()


class Quantize(FieldMarker):
    """Round an integer, a ``datetime`` or a ``timedelta`` down to a
    multiple of ``step``: nanoseconds for times, so ``Quantize(10**9)`` is
    to the second."""

    __slots__ = ("step",)
    _options = ("step",)

    def __init__(self, step: int, *identities: str) -> None:
        if type(step) is not int or step < 1:
            raise TypeError(f"Quantize takes a positive integer, not {step!r}")
        super().__init__(*identities)
        self.step = step


class Decimals(FieldMarker):
    """Round a number down to ``places`` decimal places (to hundreds with
    ``-2``), taking a float as the shortest decimal that reads back as it."""

    __slots__ = ("places",)
    _options = ("places",)

    def __init__(self, places: int, *identities: str) -> None:
        if type(places) is not int or not -300 <= places <= 300:
            raise TypeError(f"Decimals takes an integer from -300 to 300, not {places!r}")
        super().__init__(*identities)
        self.places = places


class AsSet(FieldMarker):
    """Digest a list or tuple as a set: the order of its items does not
    count. Repeated items are kept."""

    __slots__ = ()


class Flatten(FieldMarker):
    """Write the members of a nested mapping or declared class into the
    containing class's map, as serde's ``flatten``."""

    __slots__ = ()


class Nested(FieldMarker):
    """Digest a nested declared class under its identity ``identity``,
    rather than its primary identity."""

    __slots__ = ("identity",)
    _options = ("identity",)

    def __init__(self, identity: str, *identities: str) -> None:
        if not isinstance(identity, str) or not identity:
            raise TypeError(f"Nested takes an identity name, not {identity!r}")
        super().__init__(*identities)
        self.identity = identity


FIELD_MARKERS: tuple[type[FieldMarker], ...] = (
    Skip,
    Rename,
    OmitNone,
    OmitEmpty,
    Quantize,
    Decimals,
    AsSet,
    Flatten,
    Nested,
)


class KindMarker:
    """An option of a kind that its type cannot say."""

    __slots__ = ()

    def __eq__(self, other: object) -> bool:
        return type(other) is type(self) and repr(other) == repr(self)

    def __hash__(self) -> int:
        return hash(repr(self))


class Unit(KindMarker):
    """The unit of a number read as an instant or a duration:
    ``Annotated[datetime, Unit("ms")]`` reads ``1700000000000`` as an
    instant in milliseconds."""

    __slots__ = ("unit",)

    def __init__(self, unit: Literal["s", "ms", "us", "ns"]) -> None:
        if unit not in ("s", "ms", "us", "ns"):
            raise TypeError(f'a unit is "s", "ms", "us" or "ns", not {unit!r}')
        self.unit = unit

    def __repr__(self) -> str:
        return f"Unit({self.unit!r})"


class Hex(KindMarker):
    """Bytes written as hex text, rather than base64:
    ``Annotated[bytes, Hex()]``."""

    __slots__ = ()

    def __repr__(self) -> str:
        return "Hex()"


class Tagged(KindMarker):
    """A union of declared classes told apart by a tag member, as pydantic's
    discriminated unions are: ``Annotated[Circle | Square, Tagged("type")]``,
    where each class has a field ``type`` whose type is a ``Literal`` naming
    its variant (``type: Literal["circle"]``). This is an internally tagged
    union (spec 5.8), and the classes' own digests agree with it."""

    __slots__ = ("tag",)

    def __init__(self, tag: str) -> None:
        if not isinstance(tag, str) or not tag:
            raise TypeError(f"Tagged takes the tag member's name, not {tag!r}")
        self.tag = tag

    def __repr__(self) -> str:
        return f"Tagged({self.tag!r})"


def markers(metadata: tuple[Any, ...]) -> tuple[list[FieldMarker], list[KindMarker]]:
    """The field and kind markers among ``Annotated`` metadata; other
    metadata (pydantic's, for example) is ignored."""
    fields = [m for m in metadata if isinstance(m, FieldMarker)]
    kinds = [m for m in metadata if isinstance(m, KindMarker)]
    return fields, kinds
