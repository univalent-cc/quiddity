"""An implementation of QUIDDITY_SPEC.md sections 2 to 4 (data model,
encoding and digests), used by generate.py to compute the test vectors.
Written for clarity, not speed; where it and the spec disagree, the spec
wins. Python values map onto the data model as:

    None              null
    bool              boolean
    int               integer
    float             number (normalized by spec 2.2)
    str               string: the text's WTF-8 bytes (spec 2.3)
    bytes             string
    list              list
    Set([...])        set (a multiset: duplicates are kept)
    Map([(k, v)...])  map (a multiset of entries: duplicate keys are kept)
    dict              map with distinct keys

Usage as a library: enc(v), digest(v), in_domain(d, label), text(d),
wtf8(s).
"""

import hashlib
import math
import struct

T_NULL, T_FALSE, T_TRUE = 0x01, 0x02, 0x03
T_INT, T_FLOAT = 0x10, 0x11
T_STR = 0x20
T_LIST, T_SET, T_MAP = 0x30, 0x31, 0x32

H_VALUE = b"quiddity/1/value"
H_DOMAIN = b"quiddity/1/domain"

CANONICAL_NAN = bytes.fromhex("7ff8000000000000")


class Set:
    def __init__(self, items):
        self.items = list(items)


class Map:
    def __init__(self, entries):
        self.entries = list(entries)


def entries(m):
    return m.entries if isinstance(m, Map) else list(m.items())


def u64be(n):
    return n.to_bytes(8, "big")


def der_int(n):
    """Minimal two's-complement big-endian bytes (spec 3.4)."""
    length = 1
    while True:
        try:
            return n.to_bytes(length, "big", signed=True)
        except OverflowError:
            length += 1


def norm(v):
    """Spec 2.2: a float exactly equal to an integer is that integer."""
    if isinstance(v, float) and math.isfinite(v) and v == int(v):
        return int(v)
    return v


def wtf8(s):
    """Text as bytes (spec 2.3): UTF-8, with surrogate pairs combined and
    each remaining surrogate written as UTF-8 would write its code point."""
    out = []
    i = 0
    while i < len(s):
        c = ord(s[i])
        if 0xD800 <= c < 0xDC00 and i + 1 < len(s) and 0xDC00 <= ord(s[i + 1]) < 0xE000:
            c = 0x10000 + ((c - 0xD800) << 10) + (ord(s[i + 1]) - 0xDC00)
            i += 1
        out.append(chr(c).encode("utf-8", "surrogatepass"))
        i += 1
    return b"".join(out)


def sorted_members(items):
    """Set members in canonical order (spec 2.5): by their encodings."""
    return sorted(items, key=enc)


def sorted_entries(m):
    """Map entries in canonical order (spec 2.5): by E(k), then E(v)."""
    return sorted(entries(m), key=lambda kv: (enc(kv[0]), enc(kv[1])))


def enc(v):
    """The encoding E(v) (spec 3)."""
    v = norm(v)
    if v is None:
        return bytes([T_NULL])
    if v is False:
        return bytes([T_FALSE])
    if v is True:
        return bytes([T_TRUE])
    if isinstance(v, int):
        c = der_int(v)
        return c + u64be(len(c)) + bytes([T_INT])
    if isinstance(v, float):
        b = CANONICAL_NAN if math.isnan(v) else struct.pack(">d", v)
        return b + bytes([T_FLOAT])
    if isinstance(v, str):
        v = wtf8(v)
    if isinstance(v, bytes):
        return v + u64be(len(v)) + bytes([T_STR])
    if isinstance(v, list):
        return b"".join(map(enc, v)) + u64be(len(v)) + bytes([T_LIST])
    if isinstance(v, Set):
        m = sorted_members(v.items)
        return b"".join(map(enc, m)) + u64be(len(m)) + bytes([T_SET])
    if isinstance(v, (dict, Map)):
        e = sorted_entries(v)
        body = b"".join(enc(k) + enc(x) for k, x in e)
        return body + u64be(len(e)) + bytes([T_MAP])
    raise TypeError(f"not a data-model value: {v!r}")


def digest(v):
    """Structural digest S(v) (spec 4.1)."""
    return hashlib.sha256(H_VALUE + enc(v)).digest()


def in_domain(d, label):
    """D(d, label) (spec 4.2)."""
    lb = wtf8(label) if isinstance(label, str) else label
    if not lb:
        raise ValueError("domain label must be non-empty")
    return hashlib.sha256(H_DOMAIN + lb + u64be(len(lb)) + d).digest()


def text(d):
    """Text form (spec 4.4) of a 32-, 16- or 8-byte digest."""
    prefix = {32: "sha256:", 16: "sha256-128:", 8: "sha256-64:"}[len(d)]
    return prefix + d.hex()
