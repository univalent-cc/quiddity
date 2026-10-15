"""An implementation of QUIDDITY_SPEC.md section 5 (identities), used by
generate.py to compute the identity test vectors. Written for clarity, not
speed; where it and the spec disagree, the spec wins.

Source values are Python values as json.loads gives them (with the number
rule of spec 2.7, see parse_json): None, bool, int or float (a number), str,
list (an array) and dict (an object). Data-model results are the values
qref.enc takes.

Identities are Python values as json.loads gives them for their JSON form
(spec 5.3): dict, list, str, and int for integers.

Usage as a library: load(identity) validates an identity and returns it in
canonical form; apply(identity, source) gives the data-model value;
digest(identity, source) the digest; identity_digest(identity, meta) the
digest of an identity under the identity of identities (spec Appendix C).
"""

import base64
import json
import math
import re
from fractions import Fraction

import qref
from qref import Map, Set


class InvalidIdentity(Exception):
    """The identity is not valid (spec 5.9)."""


class ApplyError(Exception):
    """The source value does not have the shape the identity describes."""


def parse_json(text):
    """A JSON text as a source value, with numbers as spec 2.7 gives them."""
    def number_int(t):
        n = int(t)
        return n if -(2**63) <= n <= 2**64 - 1 else qref.norm(float(t))

    def object_pairs(pairs):
        keys = [k for k, _ in pairs]
        if len(set(keys)) != len(keys):
            raise ValueError("duplicate member names")
        return dict(pairs)

    return json.loads(
        text, parse_int=number_int, parse_float=lambda t: qref.norm(float(t)),
        object_pairs_hook=object_pairs,
    )


# --- Kinds -----------------------------------------------------------------

SCALARS = {"null", "bool", "integer", "number", "string", "uuid", "path", "any"}
COMPOUND = {"bytes", "instant", "duration", "list", "set", "tuple", "map",
            "object", "union", "tagged", "ref"}
SOURCE_TYPES = {"null", "boolean", "number", "string", "array", "object",
                "bytes", "set", "instant", "duration"}
UNITS = {"s": 10**9, "ms": 10**6, "us": 10**3, "ns": 1}
STRUCTURAL = {"list", "set", "tuple", "map", "object", "tagged"}


def kind_name(k):
    if isinstance(k, str):
        if k not in SCALARS:
            raise InvalidIdentity(f"unknown kind {k!r}")
        return k
    if isinstance(k, dict) and len(k) == 1:
        (name,) = k
        if name not in COMPOUND:
            raise InvalidIdentity(f"unknown kind {name!r}")
        return name
    raise InvalidIdentity(f"not a kind: {k!r}")


def members(obj, where, required=(), optional=()):
    """Checks an object's members: all required present, none unknown."""
    if not isinstance(obj, dict):
        raise InvalidIdentity(f"{where}: expected an object")
    for m in required:
        if m not in obj:
            raise InvalidIdentity(f"{where}: missing member {m!r}")
    for m in obj:
        if m not in required and m not in optional:
            raise InvalidIdentity(f"{where}: unknown member {m!r}")


def one_of(value, allowed, where):
    if value not in allowed:
        raise InvalidIdentity(f"{where}: {value!r} is not one of {sorted(allowed)}")


def nonempty_str(value, where):
    if not isinstance(value, str) or not value:
        raise InvalidIdentity(f"{where}: expected a non-empty string")


def canon_key(k):
    """A string that is equal for kinds equal in canonical form."""
    return json.dumps(k, sort_keys=True, ensure_ascii=False)


# --- Loading: validation and canonical form (spec 5.3, 5.9) ----------------

def canonical_kind(k, where):
    """Validates a kind's syntax and returns it in canonical form."""
    name = kind_name(k)
    if isinstance(k, str):
        return k
    v = k[name]
    if name == "bytes":
        members(v, where, optional=("text",))
        out = {}
        if "text" in v:
            one_of(v["text"], {"base64", "hex"}, where)
            if v["text"] != "base64":
                out["text"] = v["text"]
        return {"bytes": out}
    if name in ("instant", "duration"):
        members(v, where, optional=("unit",))
        out = {}
        if "unit" in v:
            one_of(v["unit"], set(UNITS), where)
            out["unit"] = v["unit"]
        return {name: out}
    if name in ("list", "set"):
        return {name: canonical_kind(v, f"{where}.{name}")}
    if name == "tuple":
        if not isinstance(v, list) or not v:
            raise InvalidIdentity(f"{where}: a tuple needs a non-empty array of kinds")
        return {"tuple": [canonical_kind(x, f"{where}.tuple[{i}]") for i, x in enumerate(v)]}
    if name == "map":
        members(v, where, required=("key", "value"))
        return {"map": {"key": canonical_kind(v["key"], f"{where}.key"),
                        "value": canonical_kind(v["value"], f"{where}.value")}}
    if name == "object":
        return {"object": canonical_object(v, where)}
    if name == "union":
        if not isinstance(v, list) or not v:
            raise InvalidIdentity(f"{where}: a union needs a non-empty array of kinds")
        alts = {}
        for i, x in enumerate(v):
            c = canonical_kind(x, f"{where}.union[{i}]")
            # Flatten nested unions and merge equal alternatives (spec 5.7).
            for a in (c["union"] if isinstance(c, dict) and "union" in c else [c]):
                alts[canon_key(a)] = a
        ordered = [alts[key] for key in sorted(alts)]
        return ordered[0] if len(ordered) == 1 else {"union": ordered}
    if name == "tagged":
        return {"tagged": canonical_tagged(v, where)}
    if name == "ref":
        nonempty_str(v, where)
        return k
    raise AssertionError(name)


def canonical_object(v, where):
    members(v, where, required=("fields",), optional=("unlisted", "order"))
    if not isinstance(v["fields"], dict):
        raise InvalidIdentity(f"{where}: fields must be an object")
    fields = {}
    keys = set()
    for fname, f in v["fields"].items():
        fw = f"{where}.fields.{fname}"
        members(f, fw, required=("kind",), optional=("rename", "omit", "quantize", "decimals"))
        out = {"kind": canonical_kind(f["kind"], fw)}
        if "rename" in f:
            nonempty_str(f["rename"], fw)
            if f["rename"] != fname:
                out["rename"] = f["rename"]
        key = out.get("rename", fname)
        if key in keys:
            raise InvalidIdentity(f"{fw}: two fields have the key {key!r}")
        keys.add(key)
        if "omit" in f:
            omit = f["omit"]
            if not isinstance(omit, list) or len(set(map(str, omit))) != len(omit):
                raise InvalidIdentity(f"{fw}: omit must be an array of distinct conditions")
            for c in omit:
                one_of(c, {"none", "empty"}, fw)
            if omit:
                out["omit"] = sorted(omit)
        if "quantize" in f:
            q = f["quantize"]
            if not isinstance(q, int) or isinstance(q, bool) or q < 1:
                raise InvalidIdentity(f"{fw}: quantize must be a positive integer")
            if q != 1:
                out["quantize"] = q
        if "decimals" in f:
            d = f["decimals"]
            if not isinstance(d, int) or isinstance(d, bool) or not -300 <= d <= 300:
                raise InvalidIdentity(f"{fw}: decimals must be an integer from -300 to 300")
            out["decimals"] = d
        if "quantize" in f and "decimals" in f:
            raise InvalidIdentity(f"{fw}: a field has quantize or decimals, not both")
        fields[fname] = out
    out = {"fields": fields}
    if "unlisted" in v:
        one_of(v["unlisted"], {"skip"}, where)
    if "order" in v:
        order = v["order"]
        if not isinstance(order, list) or sorted(order) != sorted(fields) or len(set(order)) != len(order):
            raise InvalidIdentity(f"{where}: order must name every field exactly once")
        if any("omit" in f for f in fields.values()):
            raise InvalidIdentity(f"{where}: a field with omit in an object with order")
        out["order"] = order
    return out


def canonical_tagged(v, where):
    members(v, where, required=("variants",),
            optional=("style", "tag", "content", "encode_as"))
    style = v.get("style", "external")
    one_of(style, {"external", "internal", "adjacent"}, where)
    encode_as = v.get("encode_as", "string")
    one_of(encode_as, {"string", "int"}, where)
    if style == "external" and ("tag" in v or "content" in v):
        raise InvalidIdentity(f"{where}: tag and content are not allowed with external")
    if style in ("internal", "adjacent"):
        if "tag" not in v:
            raise InvalidIdentity(f"{where}: {style} needs tag")
        nonempty_str(v["tag"], where)
    if style == "internal" and "content" in v:
        raise InvalidIdentity(f"{where}: content is not allowed with internal")
    if style == "adjacent":
        if "content" not in v:
            raise InvalidIdentity(f"{where}: adjacent needs content")
        nonempty_str(v["content"], where)
        if v["content"] == v["tag"]:
            raise InvalidIdentity(f"{where}: content must differ from tag")
    variants = v["variants"]
    if not isinstance(variants, dict) or not variants:
        raise InvalidIdentity(f"{where}: variants must be a non-empty object")
    out_variants = {}
    labels = set()
    for vname, var in variants.items():
        vw = f"{where}.variants.{vname}"
        members(var, vw, optional=("kind", "rename", "discriminant"))
        out = {}
        if "kind" in var:
            out["kind"] = canonical_kind(var["kind"], vw)
        if "rename" in var:
            nonempty_str(var["rename"], vw)
            if var["rename"] != vname:
                out["rename"] = var["rename"]
        if encode_as == "int":
            d = var.get("discriminant")
            if not isinstance(d, int) or isinstance(d, bool):
                raise InvalidIdentity(f"{vw}: encode_as int needs an integer discriminant")
            if not -(2**63) <= d <= 2**64 - 1:
                raise InvalidIdentity(f"{vw}: a discriminant is a 64-bit integer")
            out["discriminant"] = d
            label = ("int", d)
        else:
            if "discriminant" in var:
                raise InvalidIdentity(f"{vw}: discriminant is allowed only with encode_as int")
            label = ("str", out.get("rename", vname))
        if label in labels:
            raise InvalidIdentity(f"{vw}: two variants have the label {label[1]!r}")
        labels.add(label)
        out_variants[vname] = out
    out = {"variants": out_variants}
    if style != "external":
        out["style"] = style
        out["tag"] = v["tag"]
    if style == "adjacent":
        out["content"] = v["content"]
    if encode_as != "string":
        out["encode_as"] = encode_as
    return out


def load(doc):
    """Validates an identity (spec 5.9) and returns its canonical form
    (spec 5.3)."""
    members(doc, "identity", required=("root",), optional=("domain", "definitions"))
    out = {}
    if "domain" in doc:
        nonempty_str(doc["domain"], "domain")
        out["domain"] = doc["domain"]
    out["root"] = canonical_kind(doc["root"], "root")
    defs = doc.get("definitions", {})
    if not isinstance(defs, dict):
        raise InvalidIdentity("definitions must be an object")
    canon_defs = {}
    for name, k in defs.items():
        nonempty_str(name, "definition name")
        canon_defs[name] = canonical_kind(k, f"definitions.{name}")
    if canon_defs:
        out["definitions"] = canon_defs
    check_semantics(out)
    # Definitions the root does not reach are not part of the identity.
    reached = set()
    reach(out["root"], canon_defs, reached)
    canon_defs = {n: k for n, k in canon_defs.items() if n in reached}
    if canon_defs:
        out["definitions"] = canon_defs
    else:
        out.pop("definitions", None)
    return out


def reach(k, defs, reached):
    """Adds the names of the definitions k reaches to reached."""
    if isinstance(k, str):
        return
    (name, v), = k.items()
    if name == "ref":
        if v not in reached:
            reached.add(v)
            reach(defs[v], defs, reached)
    elif name in ("list", "set"):
        reach(v, defs, reached)
    elif name in ("tuple", "union"):
        for x in v:
            reach(x, defs, reached)
    elif name == "map":
        reach(v["key"], defs, reached)
        reach(v["value"], defs, reached)
    elif name == "object":
        for f in v["fields"].values():
            reach(f["kind"], defs, reached)
    elif name == "tagged":
        for var in v["variants"].values():
            if "kind" in var:
                reach(var["kind"], defs, reached)


def check_semantics(doc):
    """The checks of spec 5.9 that need the whole identity: references,
    cycles, unions, map keys and internally tagged variants."""
    defs = doc.get("definitions", {})
    for k in [doc["root"], *defs.values()]:
        walk(k, defs)


def walk(k, defs):
    name = kind_name(k)
    if isinstance(k, str):
        return
    v = k[name]
    if name == "ref":
        if v not in defs:
            raise InvalidIdentity(f"ref to undefined {v!r}")
        claims(k, defs)  # detects cycles without structure
    elif name in ("list", "set"):
        walk(v, defs)
    elif name == "tuple":
        for x in v:
            walk(x, defs)
    elif name == "map":
        walk(v["key"], defs)
        walk(v["value"], defs)
        key_alts = alternatives(v["key"], defs)
        accepting = [a for a in key_alts if "string" in set().union(*claims(a, defs))]
        if not accepting:
            raise InvalidIdentity("a map's key kind must accept strings")
        if len(accepting) > 1:
            raise InvalidIdentity("a map's key union has two alternatives that accept strings")
    elif name == "object":
        for fname, f in v["fields"].items():
            walk(f["kind"], defs)
            for option, kinds in (("quantize", {"integer", "instant", "duration"}), ("decimals", {"number"})):
                if option in f and quantized_kind(f["kind"], defs) not in kinds:
                    raise InvalidIdentity(f"field {fname!r}: {option} does not apply to its kind")
    elif name == "union":
        for a in v:
            walk(a, defs)
        union_claims(v, defs)
    elif name == "tagged":
        for var in v["variants"].values():
            if "kind" in var:
                walk(var["kind"], defs)
        if v.get("style") == "internal":
            for vname, var in v["variants"].items():
                if "kind" not in var:
                    continue
                obj = resolve(var["kind"], defs)
                if not (isinstance(obj, dict) and "object" in obj):
                    raise InvalidIdentity(f"internally tagged variant {vname!r} is not an object")
                o = obj["object"]
                if "order" in o:
                    raise InvalidIdentity(f"internally tagged variant {vname!r} has order")
                tag = v["tag"]
                for fname, f in o["fields"].items():
                    if tag in (fname, f.get("rename", fname)):
                        raise InvalidIdentity(f"variant {vname!r} has a field named {tag!r}")


def quantized_kind(k, defs):
    """The name of the kind a quantization applies to: the field's kind, or
    the one alternative besides null of a union; references followed."""
    k = resolve(k, defs)
    if isinstance(k, dict) and "union" in k:
        others = [a for a in k["union"] if resolve(a, defs) != "null"]
        if len(others) != 1:
            return None
        k = resolve(others[0], defs)
    return kind_name(k)


def quantize(v, f):
    """A field's value, quantized as its `quantize` or `decimals` says
    (spec 5.6): rounded down to a multiple of the step, exactly."""
    if v is None:
        return v
    if "quantize" in f:
        return (v // f["quantize"]) * f["quantize"]
    if "decimals" in f:
        if isinstance(v, float) and (math.isnan(v) or math.isinf(v)):
            return v
        d = f["decimals"]
        scale = Fraction(10) ** d
        # A float stands for the shortest decimal that rounds to it, which is
        # what a JSON writer emits (Python's repr).
        exact = Fraction(repr(v)) if isinstance(v, float) else Fraction(v)
        r = Fraction(math.floor(exact * scale)) / scale
        return int(r) if r.denominator == 1 else qref.norm(float(r))
    return v


def resolve(k, defs, seen=()):
    """A kind with references followed."""
    while isinstance(k, dict) and "ref" in k:
        if k["ref"] in seen:
            raise InvalidIdentity(f"a cycle of references through {k['ref']!r}")
        seen = (*seen, k["ref"])
        k = defs[k["ref"]]
    return k


def alternatives(k, defs):
    """A kind's union alternatives, or the kind itself."""
    r = resolve(k, defs)
    return r["union"] if isinstance(r, dict) and "union" in r else [k]


def claims(k, defs, visiting=()):
    """The source types a kind accepts: (natively, by conversion)."""
    name = kind_name(k)
    if name == "ref":
        ref = k["ref"]
        if ref in visiting:
            raise InvalidIdentity(f"a cycle of references through {ref!r} without structure")
        return claims(defs[ref], defs, (*visiting, ref))
    if name == "union":
        return union_claims(k["union"], defs, visiting)
    table = {
        "null": ({"null"}, set()), "bool": ({"boolean"}, {"string"}),
        "integer": ({"number"}, {"string"}), "number": ({"number"}, {"string"}),
        "string": ({"string"}, set()), "uuid": ({"string"}, set()),
        "path": ({"string"}, {"object"}), "any": (set(SOURCE_TYPES), set()),
        "list": ({"array"}, set()), "set": ({"array", "set"}, set()),
        "tuple": ({"array"}, set()), "map": ({"object"}, set()),
        "object": ({"object"}, set()), "bytes": ({"bytes"}, {"array", "string"}),
    }
    if name in table:
        return table[name]
    if name in ("instant", "duration"):
        conv = {"string", "object"} | ({"number"} if "unit" in k[name] else set())
        return {name}, conv
    if name == "tagged":
        t = k["tagged"]
        if t.get("style", "external") != "external":
            return {"object"}, set()
        native = set()
        for var in t["variants"].values():
            native.add("object" if "kind" in var else "string")
        return native, set()
    raise AssertionError(name)


def union_claims(alts, defs, visiting=()):
    """A union's claims, checking that it is valid (spec 5.7)."""
    native, conv = set(), set()
    objects = []
    for a in alts:
        n, c = claims(a, defs, visiting)
        overlap = native & n
        if overlap == {"object"} and object_alternatives(alts, defs) is not None:
            overlap = set()  # told apart by their keys; checked below
        if overlap:
            raise InvalidIdentity(f"two union alternatives accept {sorted(overlap)} natively")
        if conv & c:
            raise InvalidIdentity(f"two union alternatives accept {sorted(conv & c)} by conversion")
        native |= n
        conv |= c
    objs = object_alternatives(alts, defs)
    if objs is not None and len(objs) > 1:
        for i, a in enumerate(objs):
            for b in objs[i + 1:]:
                if required(a, defs) <= set(a_fields(b)) and required(b, defs) <= set(a_fields(a)):
                    raise InvalidIdentity(
                        "two object alternatives of a union could both fit one object: "
                        f"{sorted(a_fields(a))} and {sorted(a_fields(b))}")
    return native, conv


def object_alternatives(alts, defs):
    """The object kinds among a union's alternatives that accept objects
    natively, if they are all object kinds (or refs to them); else None."""
    objs = []
    for a in alts:
        r = resolve(a, defs)
        if isinstance(r, dict) and "object" in r:
            objs.append(r["object"])
        elif "object" in claims(a, defs)[0]:
            return None
    return objs


def a_fields(o):
    return list(o["fields"])


def required(o, defs):
    """The fields of object kind o whose kinds do not accept null."""
    return {n for n, f in o["fields"].items() if "null" not in claims(f["kind"], defs)[0]}


def fits(o, s, defs):
    """Whether source object s fits object kind o (spec 5.7)."""
    return set(s) <= set(o["fields"]) and required(o, defs) <= set(s)


# --- Applying an identity (spec 5.4 to 5.8) --------------------------------

def source_type(s):
    if s is None:
        return "null"
    if isinstance(s, bool):
        return "boolean"
    if isinstance(s, (int, float)):
        return "number"
    if isinstance(s, str):
        return "string"
    if isinstance(s, list):
        return "array"
    if isinstance(s, dict):
        return "object"
    raise TypeError(f"not a source value: {s!r}")


WS = " \t\n\r"
DECIMAL = re.compile(r"[+-]?(?:0|[1-9][0-9]*)(?:\.[0-9]+)?(?:[eE][+-]?[0-9]+)?\Z")


def decimal(text):
    """Decimal text (spec 5.5) as an exact Fraction, or None."""
    return Fraction(text) if DECIMAL.match(text) else None


def json_number_value(text):
    """The value spec 2.7 gives decimal text as a JSON number."""
    body = text[1:] if text.startswith("+") else text
    if re.fullmatch(r"-?(?:0|[1-9][0-9]*)", body):
        n = int(body)
        if -(2**63) <= n <= 2**64 - 1:
            return n
    return qref.norm(float(body))


BOOLS = {"true": True, "t": True, "yes": True, "y": True, "on": True, "1": True,
         "false": False, "f": False, "no": False, "n": False, "off": False, "0": False}
NUMBER_TOKENS = {
    "nan": float("nan"), "+nan": float("nan"), "-nan": float("nan"),
    "inf": float("inf"), "+inf": float("inf"), "infinity": float("inf"),
    "+infinity": float("inf"), "∞": float("inf"), "+∞": float("inf"),
    "-inf": float("-inf"), "-infinity": float("-inf"), "-∞": float("-inf"),
}
UUID = re.compile(r"[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}\Z")
UUID_SIMPLE = re.compile(r"[0-9a-f]{32}\Z")
RFC3339 = re.compile(
    r"(\d{4})-(\d{2})-(\d{2})[Tt ](\d{2}):(\d{2}):(\d{2})(?:\.(\d{1,9}))?"
    r"(?:([Zz])|([+-])(\d{2})(?::?(\d{2}))?)\Z"
)
ISO_DURATION = re.compile(
    r"([+-])?[Pp](?:(\d+)[Ww])?(?:(\d+)[Dd])?"
    r"(?:[Tt](?:(\d+)[Hh])?(?:(\d+)[Mm])?(?:(\d+)(?:\.(\d{1,9}))?[Ss])?)?\Z"
)


def days_from_civil(y, m, d):
    """Days since 1970-01-01 in the proleptic Gregorian calendar."""
    y -= m <= 2
    era = y // 400
    yoe = y - era * 400
    doy = (153 * (m + (-3 if m > 2 else 9)) + 2) // 5 + d - 1
    doe = yoe * 365 + yoe // 4 - yoe // 100 + doy
    return era * 146097 + doe - 719468


def days_in_month(y, m):
    if m == 2:
        return 29 if (y % 4 == 0 and y % 100 != 0) or y % 400 == 0 else 28
    return 30 if m in (4, 6, 9, 11) else 31


def parse_rfc3339(text):
    m = RFC3339.match(text)
    if not m:
        raise ApplyError(f"not an RFC 3339 date-time with an offset: {text!r}")
    y, mo, d, h, mi, s = (int(g) for g in m.groups()[:6])
    frac = m.group(7) or ""
    if not (1 <= mo <= 12 and 1 <= d <= days_in_month(y, mo) and h <= 23 and mi <= 59):
        raise ApplyError(f"date or time out of range: {text!r}")
    if s >= 60:
        raise ApplyError(f"second out of range (leap seconds are errors): {text!r}")
    offset = 0
    if m.group(9):
        oh, om = int(m.group(10)), int(m.group(11) or 0)
        if oh > 23 or om > 59:
            raise ApplyError(f"offset out of range: {text!r}")
        offset = (oh * 3600 + om * 60) * (-1 if m.group(9) == "-" else 1)
    secs = days_from_civil(y, mo, d) * 86400 + h * 3600 + mi * 60 + s - offset
    return secs * 10**9 + int(frac.ljust(9, "0") or 0)


def parse_duration(text):
    m = ISO_DURATION.match(text)
    if not m or not any(m.groups()[1:6]) or (re.search(r"[Tt]\Z", text)):
        raise ApplyError(f"not an ISO 8601 duration without years or months: {text!r}")
    sign, w, d, h, mi, s, frac = m.groups()
    secs = (int(w or 0) * 7 + int(d or 0)) * 86400 + int(h or 0) * 3600 + int(mi or 0) * 60 + int(s or 0)
    ns = secs * 10**9 + int((frac or "").ljust(9, "0") or 0)
    return -ns if sign == "-" else ns


def exact_nanos(value, unit):
    ns = Fraction(value) * UNITS[unit]
    if ns.denominator != 1:
        raise ApplyError(f"{value} {unit} is not a whole number of nanoseconds")
    return int(ns)


def time_from(s, opts, kind):
    """An instant or a duration (spec 5.5), as integer nanoseconds."""
    t = source_type(s)
    unit = opts.get("unit")
    if t == "number":
        if unit is None:
            raise ApplyError(f"a number for {kind} needs a declared unit")
        return exact_nanos(s, unit)
    if t == "string":
        text = s.strip(WS)
        if unit is not None and decimal(text) is not None:
            return exact_nanos(decimal(text), unit)
        return parse_rfc3339(text) if kind == "instant" else parse_duration(text)
    if t == "object":
        names = ("secs_since_epoch", "nanos_since_epoch") if kind == "instant" else ("secs", "nanos")
        if set(s) != set(names):
            raise ApplyError(f"expected exactly the members {names}")
        secs, nanos = s[names[0]], s[names[1]]
        for x in (secs, nanos):
            if not isinstance(x, int) or isinstance(x, bool):
                raise ApplyError(f"{names} must be integers")
        if secs < 0 or not 0 <= nanos < 10**9:
            raise ApplyError(f"{names} out of range")
        return secs * 10**9 + nanos
    raise ApplyError(f"{kind} does not accept a {t}")


def is_empty(v):
    if isinstance(v, (str, bytes, list)):
        return len(v) == 0
    if isinstance(v, Set):
        return len(v.items) == 0
    if isinstance(v, Map):
        return len(v.entries) == 0
    return False


def apply_kind(k, s, defs):
    """The data-model value of source value s read with kind k."""
    name = kind_name(k)
    t = source_type(s)
    if name == "ref":
        return apply_kind(defs[k["ref"]], s, defs)
    if name == "union":
        alts = k["union"]
        objs = [a for a in alts if isinstance(resolve(a, defs), dict) and "object" in resolve(a, defs)]
        if t == "object" and len(objs) > 1:
            for a in objs:
                if fits(resolve(a, defs)["object"], s, defs):
                    return apply_kind(a, s, defs)
            for a in alts:
                if "object" in claims(a, defs)[1]:
                    return apply_kind(a, s, defs)
            raise ApplyError("the object fits none of the union's object alternatives")
        for pick in (0, 1):  # natively, then by conversion
            for a in alts:
                if t in claims(a, defs)[pick]:
                    return apply_kind(a, s, defs)
        raise ApplyError(f"no alternative of the union accepts a {t}")
    native, conv = claims(k, defs)
    if t not in native and t not in conv:
        raise ApplyError(f"{name} does not accept a {t}")
    if name == "any":
        return as_is(s)
    if name == "null":
        return None
    if name == "bool":
        if t == "boolean":
            return s
        word = s.strip(WS).lower()
        if word not in BOOLS:
            raise ApplyError(f"not a boolean: {s!r}")
        return BOOLS[word]
    if name in ("integer", "number"):
        if t == "number":
            if name == "integer" and not isinstance(s, int):
                raise ApplyError(f"not an integer: {s!r}")
            return s
        text = s.strip(WS)
        if name == "number":
            token = text.lower().replace("−", "-")
            if token in NUMBER_TOKENS:
                return NUMBER_TOKENS[token]
            if decimal(text) is None:
                raise ApplyError(f"not a number: {s!r}")
            return json_number_value(text)
        f = decimal(text)
        if f is None or f.denominator != 1:
            raise ApplyError(f"not an integer: {s!r}")
        return int(f)
    if name == "string":
        return s
    if name == "uuid":
        text = s.strip(WS).lower()
        if text.startswith("{") and text.endswith("}"):
            text = text[1:-1]
        elif text.startswith("urn:uuid:"):
            text = text[9:]
            if not UUID.match(text):
                raise ApplyError(f"not a UUID: {s!r}")
        if UUID_SIMPLE.match(text):
            text = "-".join((text[:8], text[8:12], text[12:16], text[16:20], text[20:]))
        if not UUID.match(text):
            raise ApplyError(f"not a UUID: {s!r}")
        return text
    if name == "path":
        if t == "string":
            return s
        if len(s) != 1 or next(iter(s)) not in ("Unix", "Windows"):
            raise ApplyError("a path object has exactly one member, Unix or Windows")
        (platform, units), = s.items()
        limit = 255 if platform == "Unix" else 65535
        if not isinstance(units, list) or not all(
                isinstance(u, int) and not isinstance(u, bool) and 0 <= u <= limit for u in units):
            raise ApplyError(f"{platform} must be an array of integers from 0 to {limit}")
        return bytes(units) if platform == "Unix" else qref.wtf8("".join(map(chr, units)))
    if name == "bytes":
        if t == "array":
            if not all(isinstance(b, int) and not isinstance(b, bool) and 0 <= b <= 255 for b in s):
                raise ApplyError("bytes must be integers from 0 to 255")
            return bytes(s)
        text = s.strip(WS)
        if k["bytes"].get("text") == "hex":
            if not re.fullmatch(r"(?:[0-9a-fA-F]{2})*", text):
                raise ApplyError(f"not hex: {s!r}")
            return bytes.fromhex(text)
        return base64_decode(text)
    if name in ("instant", "duration"):
        return time_from(s, k[name], name)
    if name in ("list", "set"):
        items = [apply_kind(k[name], x, defs) for x in s]
        return items if name == "list" else Set(items)
    if name == "tuple":
        if len(s) != len(k["tuple"]):
            raise ApplyError(f"expected {len(k['tuple'])} items, found {len(s)}")
        return [apply_kind(kk, x, defs) for kk, x in zip(k["tuple"], s)]
    if name == "map":
        m = k["map"]
        return Map((apply_kind(m["key"], key, defs), apply_kind(m["value"], x, defs))
                   for key, x in s.items())
    if name == "object":
        return apply_object(k["object"], s, defs)
    if name == "tagged":
        return apply_tagged(k["tagged"], s, defs)
    raise AssertionError(name)


def base64_decode(text):
    std, url = set("+/"), set("-_")
    chars = set(text.rstrip("="))
    if chars & std and chars & url:
        raise ApplyError("base64 mixes the standard and URL-safe alphabets")
    body = text.rstrip("=")
    padding = len(text) - len(body)
    if padding and (len(text) % 4 or padding > 2):
        raise ApplyError("incorrect base64 padding")
    if len(body) % 4 == 1:
        raise ApplyError("not base64")
    body = body.translate(str.maketrans("-_", "+/"))
    try:
        return base64.b64decode(body + "=" * (-len(body) % 4), validate=True)
    except ValueError as e:
        raise ApplyError(f"not base64: {e}") from None


def apply_object(o, s, defs, skip=None):
    """An object kind applied to source object s; `skip` is a member to
    leave out (an internally tagged union's tag)."""
    values = []
    for fname, f in o["fields"].items():
        v = quantize(apply_kind(f["kind"], s.get(fname), defs), f)
        omit = f.get("omit", [])
        if ("none" in omit and v is None) or ("empty" in omit and is_empty(v)):
            continue
        values.append((fname, f.get("rename", fname), v))
    if "order" in o:
        by_name = {n: v for n, _, v in values}
        return [by_name[n] for n in o["order"]]
    return Map((key, v) for _, key, v in values)


def apply_tagged(t, s, defs):
    style = t.get("style", "external")
    int_mode = t.get("encode_as") == "int"
    variants = t["variants"]

    def label(vname):
        var = variants[vname]
        return var["discriminant"] if int_mode else var.get("rename", vname)

    def variant(vname):
        if not isinstance(vname, str) or vname not in variants:
            raise ApplyError(f"unknown variant {vname!r}")
        return variants[vname]

    if style == "external":
        if isinstance(s, str):
            if "kind" in variant(s):
                raise ApplyError(f"variant {s!r} has a value")
            return label(s)
        if not isinstance(s, dict) or len(s) != 1:
            raise ApplyError("an externally tagged value is a string or an object with one member")
        (vname, x), = s.items()
        var = variant(vname)
        if "kind" not in var:
            raise ApplyError(f"unit variant {vname!r} has a value")
        v = apply_kind(var["kind"], x, defs)
        return [label(vname), v] if int_mode else Map([(label(vname), v)])
    if not isinstance(s, dict) or t["tag"] not in s:
        raise ApplyError(f"expected an object with the member {t['tag']!r}")
    vname = s[t["tag"]]
    var = variant(vname)
    tag_entry = (t["tag"], label(vname))
    if "kind" not in var:
        return Map([tag_entry])
    if style == "internal":
        obj = resolve(var["kind"], defs)["object"]
        rest = {k: x for k, x in s.items() if k != t["tag"]}
        result = apply_object(obj, rest, defs)
        return Map([*result.entries, tag_entry])
    if t["content"] not in s:
        raise ApplyError(f"expected the member {t['content']!r}")
    return Map([tag_entry, (t["content"], apply_kind(var["kind"], s[t["content"]], defs))])


def as_is(s):
    """A source value as the data-model value spec 2.7 gives it."""
    if isinstance(s, list):
        return [as_is(x) for x in s]
    if isinstance(s, dict):
        return Map((k, as_is(x)) for k, x in s.items())
    return s


def apply(identity, source):
    """The data-model value of a source value under a loaded identity."""
    return apply_kind(identity["root"], source, identity.get("definitions", {}))


def digest(identity, source):
    """The digest of a source value under a loaded identity (spec 5.1)."""
    d = qref.digest(apply(identity, source))
    return qref.in_domain(d, identity["domain"]) if "domain" in identity else d


def identity_digest(doc, meta):
    """The digest of an identity (spec 5.10): its canonical form under the
    identity of identities."""
    return digest(load(meta), load(doc))
