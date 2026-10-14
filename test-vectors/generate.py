"""Generate quiddity-v1.json, the test vectors, using qref.py.

    python3 test-vectors/generate.py          # rewrite the file
    python3 test-vectors/generate.py --check  # fail if the file is stale

Values are written in a typed JSON notation, since JSON alone cannot express
binary strings, text with surrogates, sets, non-string map keys, or a float
distinct from an integer:

    {"null": null}
    {"bool": true}
    {"int": "-129"}                       decimal, any size
    {"float": "3ff8000000000000"}         binary64 bits, before normalization
    {"string": "a"}                       text
    {"code_points": [97, 55296]}          text, which may contain surrogates
    {"bytes": "0102"}                     a string given as hex
    {"list": [v, ...]}
    {"set": [v, ...]}                     in no particular order
    {"map": [[k, v], ...]}                in no particular order
"""

import json
import pathlib
import struct
import sys

from qref import Map, Set, digest, enc, in_domain, text

OUT = pathlib.Path(__file__).resolve().parent / "quiddity-v1.json"


# Constructors for the typed notation. Each returns (python value, json form).

def null():
    return None, {"null": None}


def boolean(b):
    return b, {"bool": b}


def int_(n):
    return n, {"int": str(n)}


def float_bits(hexbits):
    return struct.unpack(">d", bytes.fromhex(hexbits))[0], {"float": hexbits}


def float_(x):
    return float_bits(struct.pack(">d", x).hex())


def string(s):
    return s, {"string": s}


def code_points(*cps):
    return "".join(map(chr, cps)), {"code_points": list(cps)}


def bytes_(hexs):
    return bytes.fromhex(hexs), {"bytes": hexs}


def list_(*items):
    return [p for p, _ in items], {"list": [j for _, j in items]}


def set_(*items):
    return Set(p for p, _ in items), {"set": [j for _, j in items]}


def map_(*pairs):
    return (
        Map((k[0], v[0]) for k, v in pairs),
        {"map": [[k[1], v[1]] for k, v in pairs]},
    )


def obj(**fields):
    return map_(*((string(k), v) for k, v in fields.items()))


VALUES = [
    # Null and booleans.
    ("null", null()),
    ("false", boolean(False)),
    ("true", boolean(True)),
    # Integers: DER minimal two's complement, any size.
    ("int 0", int_(0)),
    ("int 1", int_(1)),
    ("int -1", int_(-1)),
    ("int 127", int_(127)),
    ("int 128", int_(128)),
    ("int -128", int_(-128)),
    ("int -129", int_(-129)),
    ("int 255", int_(255)),
    ("int 256", int_(256)),
    ("int i64::MIN", int_(-(2**63))),
    ("int i64::MAX", int_(2**63 - 1)),
    ("int u64::MAX", int_(2**64 - 1)),
    ("int 2^64", int_(2**64)),
    ("int u128::MAX", int_(2**128 - 1)),
    ("int i128::MIN", int_(-(2**127))),
    ("int 2^200", int_(2**200)),
    # Floats: integral values are integers; NaN is canonical.
    ("float 1.0 (is int 1)", float_(1.0)),
    ("float -0.0 (is int 0)", float_(-0.0)),
    ("float -2.0 (is int -2)", float_(-2.0)),
    ("float 2^64 (is int 2^64)", float_(2.0**64)),
    ("float 1e300 (is an int)", float_(1e300)),
    ("float 1.5", float_(1.5)),
    ("float -1.5", float_(-1.5)),
    ("float 0.1", float_(0.1)),
    ("float f32 0.1 widened", float_(struct.unpack(">f", struct.pack(">f", 0.1))[0])),
    ("float smallest subnormal", float_bits("0000000000000001")),
    ("float +inf", float_(float("inf"))),
    ("float -inf", float_(float("-inf"))),
    ("float NaN canonical", float_bits("7ff8000000000000")),
    ("float NaN with payload", float_bits("7ff0000000000001")),
    ("float NaN negative", float_bits("fff8000000000000")),
    # Strings: UTF-8, no normalization.
    ("string empty", string("")),
    ("string a", string("a")),
    ("string e-acute precomposed", string("é")),
    ("string e-acute decomposed", string("é")),
    ("string astral", string("\U0001F600")),
    ("string nul", string("\u0000")),
    # Text with surrogates, as WTF-8; JSON cannot hold it as a string.
    ("text lone high surrogate", code_points(0xD800)),
    ("text lone low surrogate", code_points(0xDC00)),
    ("text surrogate after a scalar value", code_points(0x61, 0xDFFF)),
    ("text surrogate pair (is U+1F600)", code_points(0xD83D, 0xDE00)),
    ("text low then high surrogate", code_points(0xDC00, 0xD800)),
    # Binary strings: the same kind as text.
    ("bytes empty (is string empty)", bytes_("")),
    ("bytes 0102", bytes_("0102")),
    ("bytes 61 (is string a)", bytes_("61")),
    ("bytes ff (not UTF-8)", bytes_("ff")),
    # Lists.
    ("list empty", list_()),
    ("list [1, a]", list_(int_(1), string("a"))),
    ("list [a, 1]", list_(string("a"), int_(1))),
    ("list nested", list_(list_(), list_(null()))),
    ("list of byte-valued ints", list_(int_(1), int_(2))),
    # Sets: canonical order, duplicates kept.
    ("set empty", set_()),
    ("set {2, 1}", set_(int_(2), int_(1))),
    ("set {1, 2}", set_(int_(1), int_(2))),
    ("set {1, 1}", set_(int_(1), int_(1))),
    ("set {1, 1.0}", set_(int_(1), float_(1.0))),
    ("set of every kind", set_(
        obj(), set_(), list_(), string(""), float_bits("7ff8000000000000"),
        float_(float("inf")), float_(1.5), int_(1), float_(float("-inf")),
        boolean(True), boolean(False), null(),
    )),
    ("set {b, aa}", set_(string("b"), string("aa"))),
    ("set {ab, a}", set_(string("ab"), string("a"))),
    ("set {a, bytes 61}", set_(string("a"), bytes_("61"))),
    ("set {-1, 1} (1 first)", set_(int_(-1), int_(1))),
    ("set {1.5, 1} (1 first)", set_(float_(1.5), int_(1))),
    ("set {[1,2], [1]}", set_(list_(int_(1), int_(2)), list_(int_(1)))),
    # Maps: entries sorted by key, then value; duplicates kept.
    ("map empty", map_()),
    ("map {start, end}", obj(start=int_(10), end=int_(15))),
    ("map {end, start}", obj(end=int_(15), start=int_(10))),
    ("map {b, aa}", obj(b=int_(1), aa=int_(2))),
    ("map non-string keys", map_(
        (string("k"), null()), (int_(2), null()), (null(), null()),
        (list_(), null()), (boolean(True), null()),
    )),
    ("map keys a and a-NUL (a-NUL first)", map_((string("a"), int_(1)), (string("a\u0000"), int_(2)))),
    ("map duplicate key", map_((string("k"), int_(2)), (string("k"), int_(1)))),
    ("map nested", obj(
        to=string("bob"), amount=int_(100),
        tags=set_(string("x"), string("y")),
        memo=null(),
    )),
]

DOMAINS = [
    ("payment", obj(to=string("bob"), amount=int_(100)), "example.payment"),
    ("refund", obj(to=string("bob"), amount=int_(100)), "example.refund"),
    ("null in domain", null(), "a"),
    ("non-ascii label", null(), "café"),
]

d0 = digest(None)
TEXT_FORMS = [
    (text(d0), True, "MUST"),
    (text(d0[:16]), True, "MUST"),
    (text(d0[:8]), True, "MUST"),
    ("sha256:" + d0.hex().upper(), True, "SHOULD"),
    ("SHA256:" + d0.hex(), False, "MUST"),
    ("sha256:" + d0[:16].hex(), False, "MUST"),
    ("sha256-128:" + d0.hex(), False, "MUST"),
    ("sha256-64:" + d0[:7].hex(), False, "MUST"),
    ("sha256:" + d0.hex() + "00", False, "MUST"),
    ("sha256: " + d0.hex(), False, "MUST"),
    ("sha256:" + d0.hex()[:-1] + "g", False, "MUST"),
]


def digests(d):
    return {"digest": text(d), "digest128": text(d[:16]), "digest64": text(d[:8])}


def build():
    values = []
    for name, (py, js) in VALUES:
        values.append({"name": name, "value": js, "encoding": enc(py).hex(), **digests(digest(py))})
    domains = []
    for name, (py, js), label in DOMAINS:
        s = digest(py)
        domains.append({
            "name": name, "value": js, "label": label,
            "structural": text(s), **digests(in_domain(s, label)),
        })
    texts = [{"text": t, "valid": ok, "requirement": req} for t, ok, req in TEXT_FORMS]
    return {
        "spec": "QUIDDITY_SPEC.md, version 1 (draft)",
        "generator": "test-vectors/generate.py",
        "values": values,
        "domains": domains,
        "text_forms": texts,
    }


def main():
    out = json.dumps(build(), indent=2, ensure_ascii=True) + "\n"
    if "--check" in sys.argv[1:]:
        if not OUT.exists() or OUT.read_text() != out:
            sys.exit(f"{OUT.name} is out of date; run {sys.argv[0]}")
        return
    OUT.write_text(out)


if __name__ == "__main__":
    main()
