"""Values of the data model (spec 2) and their encoding (spec 3).

Python values are read as data-model values first (``_value.from_python``);
the encoding is computed from those. A data-model value is one of:

    None, bool, int, float     null, boolean, integer, number
    str, bytes                 string: text (its WTF-8 bytes) or bytes
    list                       list
    Set                        set: order does not count, repeats do
    Map                        map: a list of (key, value) entries
    Instant, Duration          integer nanoseconds, kept apart from integers
                               so that identities can accept them as they are

A float with an integer value is held as that integer (spec 2.2).
"""

from __future__ import annotations

import math
import struct
from typing import Any

T_NULL, T_FALSE, T_TRUE = 0x01, 0x02, 0x03
T_INT, T_FLOAT = 0x10, 0x11
T_STR = 0x20
T_LIST, T_SET, T_MAP = 0x30, 0x31, 0x32

_NAN = bytes.fromhex("7ff8000000000000")


class Set:
    """A set of the data model."""

    __slots__ = ("items",)

    def __init__(self, items: list[Any]) -> None:
        self.items = items

    def __repr__(self) -> str:
        return f"Set({self.items!r})"


class Map:
    """A map of the data model, as a list of entries."""

    __slots__ = ("entries",)

    def __init__(self, entries: list[tuple[Any, Any]]) -> None:
        self.entries = entries

    def __repr__(self) -> str:
        return f"Map({self.entries!r})"

    def get(self, key: str) -> Any:
        """The value of the entry with the text key ``key``, or None."""
        for k, v in self.entries:
            if k == key and type(k) is str:
                return v
        return None

    def has(self, key: str) -> bool:
        return any(k == key and type(k) is str for k, _ in self.entries)


class Instant:
    """An instant: integer nanoseconds since 1970-01-01T00:00:00Z (spec 2.6)."""

    __slots__ = ("ns",)

    def __init__(self, ns: int) -> None:
        self.ns = ns

    def __repr__(self) -> str:
        return f"Instant({self.ns})"

    def __eq__(self, other: object) -> bool:
        return type(other) is Instant and other.ns == self.ns

    def __hash__(self) -> int:
        return hash(("instant", self.ns))


class Duration:
    """A duration: integer nanoseconds (spec 2.6)."""

    __slots__ = ("ns",)

    def __init__(self, ns: int) -> None:
        self.ns = ns

    def __repr__(self) -> str:
        return f"Duration({self.ns})"

    def __eq__(self, other: object) -> bool:
        return type(other) is Duration and other.ns == self.ns

    def __hash__(self) -> int:
        return hash(("duration", self.ns))


def normalize(x: float) -> float | int:
    """A float as the data model holds it (spec 2.2): a finite float with an
    integer value is that integer."""
    if math.isfinite(x) and x == int(x):
        return int(x)
    return x


def wtf8(s: str) -> bytes:
    """Text as bytes (spec 2.3): UTF-8, with surrogate pairs combined and
    each lone surrogate written as UTF-8 would write its code point."""
    try:
        return s.encode("utf-8")
    except UnicodeEncodeError:
        pass
    out = []
    i = 0
    n = len(s)
    while i < n:
        c = ord(s[i])
        if 0xD800 <= c < 0xDC00 and i + 1 < n and 0xDC00 <= ord(s[i + 1]) < 0xE000:
            c = 0x10000 + ((c - 0xD800) << 10) + (ord(s[i + 1]) - 0xDC00)
            i += 1
        out.append(chr(c).encode("utf-8", "surrogatepass"))
        i += 1
    return b"".join(out)


def _int_bytes(n: int) -> bytes:
    """Minimal two's-complement big-endian bytes (spec 3.4)."""
    length = (n.bit_length() + 8) // 8 if n >= 0 else ((-n - 1).bit_length() + 8) // 8
    return n.to_bytes(length, "big", signed=True)


def encode(v: Any) -> bytes:
    """The encoding E(v) of a data-model value (spec 3)."""
    out = bytearray()
    _encode(v, out)
    return bytes(out)


def _encode(v: Any, out: bytearray) -> None:
    t = type(v)
    if v is None:
        out.append(T_NULL)
    elif t is bool:
        out.append(T_TRUE if v else T_FALSE)
    elif t is int or t is Instant or t is Duration:
        n = v if t is int else v.ns
        b = _int_bytes(n)
        out += b
        out += len(b).to_bytes(8, "big")
        out.append(T_INT)
    elif t is float:
        x = normalize(v)
        if type(x) is int:
            _encode(x, out)
            return
        out += _NAN if math.isnan(x) else struct.pack(">d", x)
        out.append(T_FLOAT)
    elif t is str or t is bytes:
        b = wtf8(v) if t is str else v
        out += b
        out += len(b).to_bytes(8, "big")
        out.append(T_STR)
    elif t is list:
        for item in v:
            _encode(item, out)
        out += len(v).to_bytes(8, "big")
        out.append(T_LIST)
    elif t is Set:
        # Canonical order (spec 2.5): by encoding.
        for e in sorted(map(encode, v.items)):
            out += e
        out += len(v.items).to_bytes(8, "big")
        out.append(T_SET)
    elif t is Map:
        # Canonical order (spec 2.5): by the key's encoding, then the value's.
        for ek, ex in sorted((encode(k), encode(x)) for k, x in v.entries):
            out += ek
            out += ex
        out += len(v.entries).to_bytes(8, "big")
        out.append(T_MAP)
    else:
        raise TypeError(f"not a data-model value: {v!r}")


def is_empty(v: Any) -> bool:
    """Whether ``v`` is an empty string, list, set or map."""
    t = type(v)
    if t is str or t is bytes or t is list:
        return len(v) == 0
    if t is Set:
        return not v.items
    if t is Map:
        return not v.entries
    return False
