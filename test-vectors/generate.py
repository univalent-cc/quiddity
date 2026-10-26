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
import re
import struct
import sys

import qident
from qref import Map, Set, digest, enc, from_json, in_domain, text

OUT = pathlib.Path(__file__).resolve().parent / "quiddity-v1.json"
SPEC = pathlib.Path(__file__).resolve().parent.parent / "QUIDDITY_SPEC.md"


def spec_json(marker):
    """The JSON example in the spec that contains `marker`."""
    blocks = re.findall(r"```json\n(.*?)```", SPEC.read_text(), re.S)
    (block,) = [b for b in blocks if marker in b]
    return json.loads(block)


# The identity of identities (spec Appendix C), and the example of
# spec 5.3, taken from the spec itself.
META = spec_json('"quiddity.identity"')
EXAMPLE = spec_json('"example.question"')


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

# JSON texts (spec 2.7). Texts in one group have the same digest; texts in
# different groups do not. The pairs marked JCS test Appendix B.
JSON = [
    # Numbers by binary64 value (JCS agrees).
    ("1", "1.0", "1e0", "10e-1", "0.1e1"),
    ("0", "-0", "-0.0", "0e5"),
    ("1.5", "15e-1"),
    # Integers are exact within 64 bits (JCS rounds above 2^53).
    ("9007199254740992", "9007199254740993.0", "9007199254740992.0"),
    ("9007199254740993",),
    ("18446744073709551615",),
    ("-9223372036854775808",),
    # Beyond 64 bits, the nearest binary64.
    ("18446744073709551616", "18446744073709551617", "1.8446744073709552e19"),
    # Member order does not matter; item order does (JCS agrees).
    ('{"a":1,"b":[true,null]}', '{ "b" : [ true , null ] , "a" : 1 }'),
    ('[1,2]',),
    ('[2,1]',),
    # Strings by code points, without normalization (JCS agrees).
    ('"\u00e9"', '"é"'),
    ('"e\u0301"',),
    ('"\ud83d\ude00"', '"😀"'),
    # Nested objects.
    ('{"to":"bob","amount":100}', '{"amount":1e2,"to":"bob"}'),
]

def doc(root, **extra):
    return {"root": root, **extra}


ERR = object()

# Identities (spec 5). Each entry: a name, an identity, and source values.
# A source is a JSON value; `ERR` marks one that MUST fail. Sources in a
# group (a tuple) MUST have the same digest; different groups MUST NOT.
IDENTITIES = [
    ("integer", doc("integer"), [
        (5, 5.0, "5", " +5 ", "5.0", "5e0", "50e-1"),
        ("18446744073709551617",),
        (-1, "-1"),
        (5.5, ERR), ("5.5", ERR), (True, ERR), ("five", ERR), (None, ERR),
    ]),
    ("number", doc("number"), [
        (1.5, "1.5", "15e-1"),
        (1, 1.0, "1"),
        ("nan", "NaN", "-NAN"),
        ("inf", "+Infinity", "\u221e"),
        ("-inf", "-\u221e", "\u2212\u221e", "\u2212INF"),
        ("abc", ERR), (True, ERR),
    ]),
    ("bool", doc("bool"), [
        (True, "true", "T", "Yes", "y", " on ", "1"),
        (False, "False", "f", "NO", "n", "off", "0"),
        ("maybe", ERR), (1, ERR), (None, ERR),
    ]),
    ("string", doc("string"), [("x",), ("X",), (5, ERR), (True, ERR)]),
    ("uuid", doc("uuid"), [
        ("67e55044-10b1-426f-9247-bb680e5fe0c8", "67E55044-10B1-426F-9247-BB680E5FE0C8",
         "67e5504410b1426f9247bb680e5fe0c8", "{67e55044-10b1-426f-9247-bb680e5fe0c8}",
         "urn:uuid:67e55044-10b1-426f-9247-bb680e5fe0c8"),
        ("67e55044-10b1-426f-9247-bb680e5fe0c", ERR),
    ]),
    ("path", doc("path"), [
        ("a/b", {"Unix": [97, 47, 98]}, {"Windows": [97, 47, 98]}),
        ({"Unix": [97, 255]},),
        ({"Windows": [97, 0xD800]},),
        ({"Windows": [0xD83D, 0xDE00]}, "\U0001F600"),
        ({"Mac": [97]}, ERR), ({"Unix": [256]}, ERR),
    ]),
    ("bytes", doc({"bytes": {}}), [
        ([1, 2], "AQI=", "AQI"),
        ([251, 255], "+/8=", "-_8"),
        ([],  ""),
        ("+_8=", ERR), ("AQI==", ERR), ([256], ERR), ("A", ERR),
    ]),
    ("bytes as hex", doc({"bytes": {"text": "hex"}}), [
        ("0102", "0102", [1, 2]),
        ("ABcd", [0xAB, 0xCD]),
        ("012", ERR),
    ]),
    ("instant", doc({"instant": {}}), [
        ("2024-01-01T00:00:00Z", "2024-01-01t00:00:00z", "2024-01-01 01:00:00+01:00",
         "2023-12-31T19:00:00-0500", "2024-01-01T02:00:00+02", "2024-01-01T00:00:00",
         {"secs_since_epoch": 1704067200, "nanos_since_epoch": 0}),
        ("1969-12-31T23:59:59Z",),
        ("2024-01-01T00:00:00.5Z", "2024-01-01T00:00:00.500000000Z"),
        ("2016-12-31T23:59:60.5Z", "2017-01-01T00:00:00.5Z", "2017-01-01T00:59:60.5+01:00"),
        ("2016-12-31T23:59:61Z", ERR),
        ("2024-02-30T00:00:00Z", ERR), ("2024-01-01T00:00:00.1234567891Z", ERR),
        (1704067200, ERR),
    ]),
    ("instant in milliseconds", doc({"instant": {"unit": "ms"}}), [
        (1704067200000, "1704067200000", "2024-01-01T00:00:00Z"),
        (1704067200000.5, "1704067200000.5"),
        ("1704067200000.0000005", ERR),
    ]),
    ("duration", doc({"duration": {}}), [
        ("PT1.5S", "pt1.5s", {"secs": 1, "nanos": 500000000}),
        ("P1W", "P7D", "PT168H", {"secs": 604800, "nanos": 0}),
        ("-PT1M",),
        ("P1Y", ERR), ("P1M", ERR), ("PT", ERR), ("P", ERR), (60, ERR),
    ]),
    ("duration in seconds", doc({"duration": {"unit": "s"}}), [
        (90, "90", "PT1M30S", "PT90S"),
        (0.25, "0.25", "PT0.25S"),
    ]),
    ("any", doc("any"), [
        ({"b": [1, 2.0, None], "a": "x"}, {"a": "x", "b": [1, 2, None]}),
        ("5",), (5,),
    ]),
    ("list, set and tuple", doc({"tuple": [{"list": "integer"}, {"set": "integer"}]}), [
        ([[1, "2"], ["2", 1]], [[1, 2], [1, 2]]),
        ([[2, 1], [1, 2]],),
        ([[1, 2], [1, 2], []], ERR), ([[1], [1, 1]],),
    ]),
    ("map with integer keys", doc({"map": {"key": "integer", "value": "string"}}), [
        ({"1": "a", "02": "b"}, ERR),
        ({"1": "a", "2": "b"}, {"2": "b", "1": "a"}, {"+1": "a", " 2": "b"}),
    ]),
    ("object", doc({"object": {"fields": {
        "id": {"kind": "integer"},
        "user_id": {"kind": "integer", "rename": "userId"},
        "note": {"kind": {"union": ["null", "string"]}, "omit": ["none"]},
        "tags": {"kind": {"union": ["null", {"set": "string"}]}, "omit": ["none", "empty"]},
        "maybe": {"kind": {"union": ["null", "integer"]}},
    }}}), [
        ({"id": 1, "user_id": 2, "note": None, "tags": [], "maybe": None},
         {"id": 1, "user_id": 2, "tags": [], "extra": True},
         {"id": "1", "user_id": 2.0}),
        ({"id": 1, "user_id": 2, "note": "n", "tags": ["b", "a"]},
         {"id": 1, "user_id": 2, "note": "n", "tags": ["a", "b"], "maybe": None}),
        ({"id": 1},  ERR), ({"id": 1, "user_id": 2, "note": 5}, ERR), ([1, 2], ERR),
    ]),
    ("object as a list", doc({"object": {"fields": {
        "a": {"kind": "integer"}, "b": {"kind": {"union": ["null", "string"]}},
    }, "order": ["b", "a"]}}), [
        ({"a": 1, "b": "x"},), ({"a": 1},  {"a": 1, "b": None}),
    ]),
    ("union of kinds", doc({"union": ["integer", "string", "null"]}), [
        (5, 5.0), ("5",), (None,), (True, ERR), ([5], ERR),
    ]),
    ("objects told apart by keys", doc({"union": [
        {"ref": "Point"}, {"ref": "Point3"}, {"ref": "Rect"}, "string",
    ]}, definitions={
        "Point": {"object": {"fields": {"x": {"kind": "integer"}, "y": {"kind": "integer"}}}},
        "Point3": {"object": {"fields": {
            "x": {"kind": "integer"}, "y": {"kind": "integer"}, "z": {"kind": "integer"}}}},
        "Rect": {"object": {"fields": {
            "w": {"kind": "integer"}, "h": {"kind": "integer"},
            "label": {"kind": {"union": ["null", "string"]}, "omit": ["none"]}}}},
    }), [
        ({"x": 1, "y": 2}, {"y": "2", "x": 1}),
        ({"x": 1, "y": 2, "z": 3},),
        ({"w": 1, "h": 2}, {"w": 1, "h": 2, "label": None}),
        ({"w": 1, "h": 2, "label": "r"},),
        ("x",),
        ({"x": 1}, ERR), ({"x": 1, "y": 2, "q": 0}, ERR), ({"w": 1, "label": "r"}, ERR), ({}, ERR),
    ]),
    ("quantize an integer", doc({"object": {"fields": {"n": {"kind": "integer", "quantize": 100}}}}), [
        ({"n": 100}, {"n": 149}, {"n": "120"}, {"n": 100.0}),
        ({"n": 0}, {"n": 51}, {"n": 99}),
        ({"n": -1}, {"n": -100}),
        ({"n": -101},),
        ({"n": None}, ERR), ({"n": 1.5}, ERR),
    ]),
    ("quantize to decimals", doc({"object": {"fields": {"x": {"kind": "number", "decimals": 2}}}}), [
        ({"x": 12.98}, {"x": 12.989}, {"x": "12.9899"}),
        ({"x": 2.67}, {"x": 2.675}),
        ({"x": -0.01}, {"x": -0.001}),
        ({"x": 0.3}, {"x": 0.30000000000000004}),
        ({"x": 1234}, {"x": 1234.001}),
        ({"x": 0}, {"x": 0.001}, {"x": -0.0}),
        ({"x": "nan"},), ({"x": "-inf"},),
    ]),
    ("quantize to tens of decimals", doc({"object": {"fields": {"x": {"kind": "number", "decimals": -2}}}}), [
        ({"x": 1200}, {"x": 1234}, {"x": 1299.99}, {"x": "1250"}),
        ({"x": -100}, {"x": -1}, {"x": -0.5}),
    ]),
    ("quantize an instant", doc({"object": {"fields": {"at": {"kind": {"instant": {}}, "quantize": 1000000000}}}}), [
        ({"at": "2024-01-01T00:00:00Z"}, {"at": "2024-01-01T00:00:00.999999999Z"},
         {"at": "2024-01-01T01:00:00.5+01:00"}),
        ({"at": "1969-12-31T23:59:59Z"}, {"at": "1969-12-31T23:59:59.5Z"}),
    ]),
    ("quantize a duration", doc({"object": {"fields": {"d": {"kind": {"duration": {}}, "quantize": 60000000000}}}}), [
        ({"d": "PT1M"}, {"d": "PT1M59.9S"}, {"d": {"secs": 100, "nanos": 0}}),
        ({"d": "PT0S"}, {"d": "PT59S"}),
    ]),
    ("quantize an optional integer", doc({"object": {"fields": {
        "n": {"kind": {"union": ["null", "integer"]}, "quantize": 10, "omit": ["none"]}}}}), [
        ({"n": 12}, {"n": 19}),
        ({}, {"n": None}),
    ]),
    ("bool or string", doc({"union": ["bool", "string"]}), [
        (True,), ("yes",), ("true",), ("TRUE",),
    ]),
    ("externally tagged", doc({"tagged": {"variants": {
        "Empty": {}, "Circle": {"kind": "number"},
        "Rect": {"kind": {"object": {"fields": {"w": {"kind": "number"}, "h": {"kind": "number"}}}},
                 "rename": "rect"},
    }}}), [
        ("Empty",), ({"Circle": 1.5}, {"Circle": "1.5"}),
        ({"Rect": {"w": 1, "h": 2}},),
        ("Circle", ERR), ({"Empty": None}, ERR), ({"Circle": 1, "Rect": {}}, ERR),
        ("Square", ERR), ("rect", ERR),
    ]),
    ("externally tagged, by discriminant", doc({"tagged": {"encode_as": "int", "variants": {
        "Empty": {"discriminant": 0}, "Circle": {"kind": "number", "discriminant": 7},
    }}}), [
        ("Empty",), ({"Circle": 1.5},),
    ]),
    ("internally tagged", doc({"tagged": {"style": "internal", "tag": "type", "variants": {
        "Empty": {}, "Rect": {"kind": {"ref": "Rect"}},
    }}}, definitions={"Rect": {"object": {"fields": {
        "w": {"kind": "number"}, "h": {"kind": "number"}}}}}), [
        ({"type": "Empty"}, {"type": "Empty", "other": 1}),
        ({"type": "Rect", "w": 1, "h": 2}, {"h": 2, "w": "1", "type": "Rect"}),
        ({"w": 1, "h": 2}, ERR), ({"type": "Square"}, ERR),
    ]),
    ("adjacently tagged", doc({"tagged": {"style": "adjacent", "tag": "t", "content": "c", "variants": {
        "Empty": {}, "Circle": {"kind": "number"},
    }}}), [
        ({"t": "Empty"},), ({"t": "Circle", "c": 1.5}, {"t": "Circle", "c": "1.5", "x": 0}),
        ({"t": "Circle"}, ERR),
    ]),
    ("domain", doc("string", domain="example.label"), [("x",)]),
    ("spec example", EXAMPLE, [
        ({"brief": "b", "rows": [{"label": "x", "value": 1}], "tags": [], "note": None,
          "asked_at": "2024-01-01T00:00:00Z", "kind": "q"},
         {"brief": "b", "rows": [{"label": "x", "value": "1.0", "extra": True}], "tags": [],
          "asked_at": {"secs_since_epoch": 1704067200, "nanos_since_epoch": 0},
          "kind": "q", "unlisted": 5}),
        ({"brief": "b", "rows": [], "asked_at": "2024-01-01T00:00:00Z", "kind": "q"}, ERR),
    ]),
]

# Identities that MUST be rejected (spec 5.9).
INVALID_IDENTITIES = [
    ("unknown member", {"root": "string", "extra": 1}),
    ("a version member", {"quiddity": 1, "root": "string"}),
    ("unknown kind", doc("text")),
    ("unknown option", doc({"bytes": {"text": "base32"}})),
    ("missing root", {}),
    ("empty domain", doc("string", domain="")),
    ("undefined ref", doc({"ref": "Row"})),
    ("cycle of refs", doc({"ref": "A"}, definitions={"A": {"ref": "B"}, "B": {"union": ["null", {"ref": "A"}]}})),
    ("union of two numbers", doc({"union": ["integer", "number"]})),
    ("union of two objects", doc({"union": [{"object": {"fields": {}}}, {"map": {"key": "string", "value": "any"}}]})),
    ("union of objects with the same fields", doc({"union": [
        {"object": {"fields": {"x": {"kind": "integer"}}}},
        {"object": {"fields": {"x": {"kind": "string"}}}}]})),
    ("union of objects one fits both", doc({"union": [
        {"object": {"fields": {"x": {"kind": "integer"}}}},
        {"object": {"fields": {"x": {"kind": "integer"}, "z": {"kind": {"union": ["null", "integer"]}}}}}]})),
    ("union of an object and a tagged union", doc({"union": [
        {"object": {"fields": {"x": {"kind": "integer"}}}},
        {"tagged": {"style": "internal", "tag": "t", "variants": {"A": {}}}}]})),
    ("union of string and uuid", doc({"union": ["string", "uuid"]})),
    ("union with any", doc({"union": ["any", "null"]})),
    ("union of two conversions", doc({"union": ["integer", "bool"]})),
    ("map key not from strings", doc({"map": {"key": "null", "value": "any"}})),
    ("map key union", doc({"map": {"key": {"union": ["integer", "string"]}, "value": "any"}})),
    ("duplicate key", doc({"object": {"fields": {"a": {"kind": "string"}, "b": {"kind": "string", "rename": "a"}}}})),
    ("order with omit", doc({"object": {"fields": {"a": {"kind": "string", "omit": ["none"]}}, "order": ["a"]}})),
    ("order incomplete", doc({"object": {"fields": {"a": {"kind": "string"}, "b": {"kind": "string"}}, "order": ["a"]}})),
    ("unknown unlisted rule", doc({"object": {"fields": {}, "unlisted": "include"}})),
    ("unknown omit condition", doc({"object": {"fields": {"a": {"kind": "string", "omit": ["zero"]}}}})),
    ("empty tuple", doc({"tuple": []})),
    ("no variants", doc({"tagged": {"variants": {}}})),
    ("internal without tag", doc({"tagged": {"style": "internal", "variants": {"A": {}}}})),
    ("adjacent without content", doc({"tagged": {"style": "adjacent", "tag": "t", "variants": {"A": {}}}})),
    ("tag with external", doc({"tagged": {"tag": "t", "variants": {"A": {}}}})),
    ("internal variant not an object", doc({"tagged": {"style": "internal", "tag": "t", "variants": {"A": {"kind": "string"}}}})),
    ("internal variant with the tag field", doc({"tagged": {"style": "internal", "tag": "t", "variants": {"A": {"kind": {"object": {"fields": {"t": {"kind": "string"}}}}}}}})),
    ("duplicate label", doc({"tagged": {"variants": {"A": {}, "B": {"rename": "A"}}}})),
    ("missing discriminant", doc({"tagged": {"encode_as": "int", "variants": {"A": {}}}})),
    ("quantize a string", doc({"object": {"fields": {"s": {"kind": "string", "quantize": 10}}}})),
    ("decimals on an integer", doc({"object": {"fields": {"n": {"kind": "integer", "decimals": 2}}}})),
    ("quantize written as a float", doc({"object": {"fields": {"n": {"kind": "integer", "quantize": 5.0}}}})),
    ("decimals written as a float", doc({"object": {"fields": {"x": {"kind": "number", "decimals": 2.0}}}})),
    ("quantize a string of digits", doc({"object": {"fields": {"n": {"kind": "integer", "quantize": "5"}}}})),
    ("discriminant written as a float", doc({"tagged": {"encode_as": "int", "variants": {"A": {"discriminant": 1.0}}}})),
    ("quantize zero", doc({"object": {"fields": {"n": {"kind": "integer", "quantize": 0}}}})),
    ("decimals out of range", doc({"object": {"fields": {"x": {"kind": "number", "decimals": 301}}}})),
    ("quantize and decimals", doc({"object": {"fields": {"x": {"kind": "number", "quantize": 10, "decimals": 2}}}})),
    ("quantize a union of two kinds", doc({"object": {"fields": {
        "n": {"kind": {"union": ["integer", "string"]}, "quantize": 10}}}})),
    ("discriminant beyond 64 bits", doc({"tagged": {"encode_as": "int", "variants": {"A": {"discriminant": 2**64}}}})),
    ("discriminant without int", doc({"tagged": {"variants": {"A": {"discriminant": 1}}}})),
]

# Pairs of identities with the same digest (spec 5.10).
SAME_IDENTITY = [
    ("defaults, order and nesting",
     doc({"object": {"fields": {
         "a": {"kind": {"union": ["null", "string"]}, "omit": ["none", "empty"]},
         "b": {"kind": {"bytes": {}}}}}}),
     {"definitions": {}, "root": {"object": {"unlisted": "skip", "fields": {
         "b": {"kind": {"bytes": {"text": "base64"}}, "rename": "b"},
         "a": {"omit": ["empty", "none"], "kind": {"union": ["string", {"union": ["null", "string"]}]}}}}}}),
    ("a union of one", doc({"union": ["null", "null"]}), doc("null")),
    ("quantize 1 is the default",
     doc({"object": {"fields": {"n": {"kind": "integer", "quantize": 1}}}}),
     doc({"object": {"fields": {"n": {"kind": "integer"}}}})),
    ("unreachable definitions",
     doc({"ref": "A"}, definitions={"A": "string", "Unused": "integer"}),
     doc({"ref": "A"}, definitions={"A": "string"})),
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
    identities = []
    for name, form, groups in IDENTITIES:
        identity = qident.load(form)
        cases, seen = [], {}

        def read(source):
            # As an implementation reads it: JSON text, mapped by spec 2.7.
            return qident.parse_json(json.dumps(source))

        for g, group in enumerate(groups):
            for source in group:
                if source is ERR:
                    continue
                if ERR in group:
                    try:
                        qident.apply(identity, read(source))
                    except qident.ApplyError:
                        cases.append({"source": source, "error": True})
                        continue
                    raise AssertionError(f"{name}: {source!r} must fail")
                value = qident.apply(identity, read(source))
                d = qident.digest(identity, read(source))
                assert seen.setdefault(d, g) == g, f"{name}: {source!r} collides with group {seen[d]}"
                first = qident.digest(identity, read(group[0]))
                assert d == first, f"{name}: {source!r} differs from {group[0]!r}"
                cases.append({"source": source, "encoding": enc(value).hex(), **digests(d)})
        identities.append({
            "name": name, "identity": form,
            "identity_digest": text(qident.identity_digest(form, META)),
            "cases": cases,
        })
    invalid_identities = []
    for name, form in INVALID_IDENTITIES:
        try:
            qident.load(form)
        except qident.InvalidIdentity:
            invalid_identities.append({"name": name, "identity": form})
        else:
            raise AssertionError(f"{name} must be rejected")
    same_identity = []
    for name, a, b in SAME_IDENTITY:
        da, db = qident.identity_digest(a, META), qident.identity_digest(b, META)
        assert da == db, name
        same_identity.append({"name": name, "identities": [a, b], "identity_digest": text(da)})
    meta_digest = text(qident.identity_digest(META, META))
    json_cases = []
    seen = {}
    for group, members in enumerate(JSON):
        for t in members:
            d = digest(from_json(t))
            assert seen.setdefault(d, group) == group, f"{t} collides with group {seen[d]}"
            assert d == digest(from_json(members[0])), f"{t} differs from {members[0]}"
            json_cases.append({"text": t, "group": group, **digests(d)})
    return {
        "spec": "QUIDDITY_SPEC.md, version 1 (draft)",
        "generator": "test-vectors/generate.py",
        "values": values,
        "domains": domains,
        "json": json_cases,
        "identity_of_identities": {"identity": META, "identity_digest": meta_digest},
        "identities": identities,
        "invalid_identities": invalid_identities,
        "same_identity": same_identity,
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
