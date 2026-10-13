"""An implementation of QUIDDITY_SPEC.md sections 2 to 4 (data model,
encoding and digests), used by generate.py to compute the test vectors.
Written for clarity, not speed; where it and the spec disagree, the spec
wins. Python values map onto the data model as:

    None              null
    bool              boolean
    int               integer
    float             number (normalized by spec 2.2)
    str               string
    bytes             bytes
    list              list
    Set([...])        set (a multiset: duplicates are kept)
    Map([(k, v)...])  map (a multiset of entries: duplicate keys are kept)
    dict              map with distinct keys

Usage as a library: enc(v), digest(v), in_domain(d, label), text(d).
"""

import functools
import hashlib
import math
import struct

T_NULL, T_FALSE, T_TRUE = 0x01, 0x02, 0x03
T_INT, T_FLOAT = 0x10, 0x11
T_STR, T_BYTES = 0x20, 0x21
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


def rank(v):
    """Kind rank for the canonical order (spec 2.5)."""
    if v is None:
        return 0
    if isinstance(v, bool):
        return 1
    if isinstance(v, (int, float)):
        return 2
    if isinstance(v, str):
        return 3
    if isinstance(v, bytes):
        return 4
    if isinstance(v, list):
        return 5
    if isinstance(v, Set):
        return 6
    if isinstance(v, (dict, Map)):
        return 7
    raise TypeError(f"not a data-model value: {v!r}")


def _sign(x):
    return (x > 0) - (x < 0)


def cmp(a, b):
    """Canonical order (spec 2.5): negative, zero or positive."""
    a, b = norm(a), norm(b)
    ra, rb = rank(a), rank(b)
    if ra != rb:
        return _sign(ra - rb)
    if ra == 0:
        return 0
    if ra == 1:
        return _sign(int(a) - int(b))
    if ra == 2:
        na = isinstance(a, float) and math.isnan(a)
        nb = isinstance(b, float) and math.isnan(b)
        if na or nb:
            return _sign(int(na) - int(nb))  # NaN is greatest
        return (a > b) - (a < b)
    if ra == 3:
        a, b = a.encode("utf-8"), b.encode("utf-8")
    if ra in (3, 4):
        return (a > b) - (a < b)
    if ra == 5:
        return cmp_seq(a, b)
    if ra == 6:
        return cmp_seq(sorted_members(a.items), sorted_members(b.items))
    return cmp_seq(
        [[k, x] for k, x in sorted_entries(a)],
        [[k, x] for k, x in sorted_entries(b)],
    )


def cmp_seq(a, b):
    for x, y in zip(a, b):
        c = cmp(x, y)
        if c:
            return c
    return _sign(len(a) - len(b))


def sorted_members(items):
    return sorted(items, key=functools.cmp_to_key(cmp))


def sorted_entries(m):
    def by_key_then_value(p, q):
        return cmp(p[0], q[0]) or cmp(p[1], q[1])

    return sorted(entries(m), key=functools.cmp_to_key(by_key_then_value))


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
        c = v.encode("utf-8")  # raises on surrogates, as spec 2.3 requires
        return c + u64be(len(c)) + bytes([T_STR])
    if isinstance(v, bytes):
        return v + u64be(len(v)) + bytes([T_BYTES])
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
    lb = label.encode("utf-8")
    if not lb:
        raise ValueError("domain label must be non-empty")
    return hashlib.sha256(H_DOMAIN + lb + u64be(len(lb)) + d).digest()


def text(d):
    """Text form (spec 4.4) of a 32-, 16- or 8-byte digest."""
    prefix = {32: "sha256:", 16: "sha256-128:", 8: "sha256-64:"}[len(d)]
    return prefix + d.hex()
