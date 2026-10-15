"""Identities in their JSON form (spec 5.3): validation (spec 5.9) and the
canonical form. Kinds are held in their JSON form: dicts, lists and
strings, as ``json.loads`` gives them."""

from __future__ import annotations

import json
from typing import Any, Final

from ._errors import InvalidIdentity

Kind = Any  # a kind in its JSON form
Defs = dict[str, Kind]

SCALARS: Final = frozenset({"null", "bool", "integer", "number", "string", "uuid", "path", "any"})
COMPOUND: Final = frozenset(
    {
        "bytes",
        "instant",
        "duration",
        "list",
        "set",
        "tuple",
        "map",
        "object",
        "union",
        "tagged",
        "ref",
    }
)
SOURCE_TYPES: Final = frozenset(
    {
        "null",
        "boolean",
        "number",
        "string",
        "array",
        "object",
        "bytes",
        "set",
        "instant",
        "duration",
    }
)
UNITS: Final = {"s": 10**9, "ms": 10**6, "us": 10**3, "ns": 1}
_MIN, _MAX = -(2**63), 2**64 - 1


def _int(x: Any) -> bool:
    """Whether ``x`` is an integer, written as one: ``5.0`` is not (spec 5.3)."""
    return type(x) is int


def _str(x: Any) -> bool:
    return type(x) is str


def kind_name(k: Kind, path: str) -> str:
    if _str(k):
        if k not in SCALARS:
            raise InvalidIdentity(f"unknown kind {k!r}", path)
        return str(k)
    if type(k) is dict and len(k) == 1:
        (name,) = k
        if name not in COMPOUND:
            raise InvalidIdentity(f"unknown kind {name!r}", path)
        return str(name)
    raise InvalidIdentity(f"not a kind: {k!r}", path)


def members(
    obj: Any, path: str, required: tuple[str, ...] = (), optional: tuple[str, ...] = ()
) -> None:
    """Checks an object's members: every required one present, none unknown."""
    if type(obj) is not dict:
        raise InvalidIdentity("expected an object", path)
    for m in required:
        if m not in obj:
            raise InvalidIdentity(f"missing member {m!r}", path)
    for m in obj:
        if m not in required and m not in optional:
            raise InvalidIdentity(f"unknown member {m!r}", path)


def _one_of(value: Any, allowed: frozenset[str] | set[str], path: str) -> None:
    if not _str(value) or value not in allowed:
        raise InvalidIdentity(f"{value!r} is not one of {', '.join(sorted(allowed))}", path)


def _nonempty(value: Any, path: str) -> None:
    if not _str(value) or not value:
        raise InvalidIdentity("expected a non-empty string", path)


def canon_key(k: Kind) -> str:
    """A string that is equal for kinds equal in canonical form."""
    return json.dumps(k, sort_keys=True, ensure_ascii=False, separators=(",", ":"))


# --- Canonical form, with syntax checked --------------------------------------------


def canonical(k: Kind, path: str) -> Kind:
    """A kind's syntax checked, in canonical form."""
    name = kind_name(k, path)
    if _str(k):
        return k
    v = k[name]
    p = f"{path}.{name}"
    if name == "bytes":
        members(v, p, optional=("text",))
        out: dict[str, Any] = {}
        if "text" in v:
            _one_of(v["text"], {"base64", "hex"}, f"{p}.text")
            if v["text"] != "base64":
                out["text"] = v["text"]
        return {"bytes": out}
    if name in ("instant", "duration"):
        members(v, p, optional=("unit",))
        out = {}
        if "unit" in v:
            _one_of(v["unit"], set(UNITS), f"{p}.unit")
            out["unit"] = v["unit"]
        return {name: out}
    if name in ("list", "set"):
        return {name: canonical(v, p)}
    if name == "tuple":
        if type(v) is not list or not v:
            raise InvalidIdentity("a tuple needs at least one kind", p)
        return {"tuple": [canonical(x, f"{p}[{i}]") for i, x in enumerate(v)]}
    if name == "map":
        members(v, p, required=("key", "value"))
        return {
            "map": {
                "key": canonical(v["key"], f"{p}.key"),
                "value": canonical(v["value"], f"{p}.value"),
            }
        }
    if name == "object":
        return {"object": _canonical_object(v, p)}
    if name == "union":
        if type(v) is not list or not v:
            raise InvalidIdentity("a union needs at least one kind", p)
        alts: dict[str, Kind] = {}
        for i, x in enumerate(v):
            c = canonical(x, f"{p}[{i}]")
            # Nested unions are flattened, and equal alternatives merged (spec 5.7).
            for a in c["union"] if type(c) is dict and "union" in c else [c]:
                alts[canon_key(a)] = a
        ordered = [alts[key] for key in sorted(alts)]
        return ordered[0] if len(ordered) == 1 else {"union": ordered}
    if name == "tagged":
        return {"tagged": _canonical_tagged(v, p)}
    # ref
    _nonempty(v, p)
    return {"ref": v}


def _canonical_object(v: Any, path: str) -> dict[str, Any]:
    members(v, path, required=("fields",), optional=("unlisted", "order"))
    if type(v["fields"]) is not dict:
        raise InvalidIdentity("expected an object", f"{path}.fields")
    fields: dict[str, Any] = {}
    keys: set[str] = set()
    for fname, f in v["fields"].items():
        fp = f"{path}.fields.{fname}"
        members(f, fp, required=("kind",), optional=("rename", "omit", "quantize", "decimals"))
        out: dict[str, Any] = {"kind": canonical(f["kind"], f"{fp}.kind")}
        if "rename" in f:
            _nonempty(f["rename"], f"{fp}.rename")
            if f["rename"] != fname:
                out["rename"] = f["rename"]
        key = str(out.get("rename", fname))
        if key in keys:
            raise InvalidIdentity(f"two fields have the key {key!r}", fp)
        keys.add(key)
        if "omit" in f:
            omit = f["omit"]
            if type(omit) is not list:
                raise InvalidIdentity("expected an array of conditions", f"{fp}.omit")
            for c in omit:
                _one_of(c, {"none", "empty"}, f"{fp}.omit")
            if len(set(omit)) != len(omit):
                raise InvalidIdentity("an omit condition is given twice", f"{fp}.omit")
            if omit:
                out["omit"] = sorted(omit)
        if "quantize" in f and "decimals" in f:
            raise InvalidIdentity("a field has quantize or decimals, not both", fp)
        if "quantize" in f:
            q = f["quantize"]
            if not _int(q) or q < 1:
                raise InvalidIdentity("quantize is a positive integer", f"{fp}.quantize")
            if q != 1:
                out["quantize"] = q
        if "decimals" in f:
            d = f["decimals"]
            if not _int(d) or not -300 <= d <= 300:
                raise InvalidIdentity("decimals is an integer from -300 to 300", f"{fp}.decimals")
            out["decimals"] = d
        fields[fname] = out
    result: dict[str, Any] = {"fields": fields}
    if "unlisted" in v:
        _one_of(v["unlisted"], {"skip"}, f"{path}.unlisted")
    if "order" in v:
        order = v["order"]
        if (
            type(order) is not list
            or not all(_str(n) for n in order)
            or sorted(order) != sorted(fields)
        ):
            raise InvalidIdentity("order names every field exactly once", f"{path}.order")
        if any("omit" in f for f in fields.values()):
            raise InvalidIdentity("a field has omit in an object with order", path)
        result["order"] = order
    return result


def _canonical_tagged(v: Any, path: str) -> dict[str, Any]:
    members(v, path, required=("variants",), optional=("style", "tag", "content", "encode_as"))
    style = v.get("style", "external")
    _one_of(style, {"external", "internal", "adjacent"}, f"{path}.style")
    encode_as = v.get("encode_as", "string")
    _one_of(encode_as, {"string", "int"}, f"{path}.encode_as")
    if style == "external" and ("tag" in v or "content" in v):
        raise InvalidIdentity("tag and content are not allowed with the external style", path)
    if style in ("internal", "adjacent"):
        if "tag" not in v:
            raise InvalidIdentity(f"the {style} style needs a tag", path)
        _nonempty(v["tag"], f"{path}.tag")
    if style == "internal" and "content" in v:
        raise InvalidIdentity("content is not allowed with the internal style", path)
    if style == "adjacent":
        if "content" not in v:
            raise InvalidIdentity("the adjacent style needs content", path)
        _nonempty(v["content"], f"{path}.content")
        if v["content"] == v["tag"]:
            raise InvalidIdentity("content differs from tag", path)
    variants = v["variants"]
    if type(variants) is not dict or not variants:
        raise InvalidIdentity("a tagged union has at least one variant", f"{path}.variants")
    out_variants: dict[str, Any] = {}
    labels: set[tuple[str, Any]] = set()
    for vname, var in variants.items():
        vp = f"{path}.variants.{vname}"
        members(var, vp, optional=("kind", "rename", "discriminant"))
        out: dict[str, Any] = {}
        if "kind" in var:
            out["kind"] = canonical(var["kind"], f"{vp}.kind")
        if "rename" in var:
            _nonempty(var["rename"], f"{vp}.rename")
            if var["rename"] != vname:
                out["rename"] = var["rename"]
        label: tuple[str, Any]
        if encode_as == "int":
            d = var.get("discriminant")
            if not _int(d):
                raise InvalidIdentity("encode_as int needs an integer discriminant", vp)
            if not _MIN <= d <= _MAX:
                raise InvalidIdentity("a discriminant is a 64-bit integer", f"{vp}.discriminant")
            out["discriminant"] = d
            label = ("int", d)
        else:
            if "discriminant" in var:
                raise InvalidIdentity("discriminant is allowed only with encode_as int", vp)
            label = ("str", out.get("rename", vname))
        if label in labels:
            raise InvalidIdentity(f"two variants have the label {label[1]!r}", vp)
        labels.add(label)
        out_variants[vname] = out
    result: dict[str, Any] = {"variants": out_variants}
    if style != "external":
        result["style"] = style
        result["tag"] = v["tag"]
    if style == "adjacent":
        result["content"] = v["content"]
    if encode_as != "string":
        result["encode_as"] = encode_as
    return result


def load(doc: Any) -> tuple[Kind, Defs, str | None]:
    """An identity's JSON form, validated (spec 5.9): its canonical root,
    definitions and domain."""
    members(doc, "$", required=("root",), optional=("domain", "definitions"))
    domain = doc.get("domain")
    if "domain" in doc:
        _nonempty(domain, "$.domain")
    defs = doc.get("definitions", {})
    if type(defs) is not dict:
        raise InvalidIdentity("expected an object", "$.definitions")
    canon_defs: Defs = {}
    for name, k in defs.items():
        _nonempty(name, "$.definitions")
        canon_defs[name] = canonical(k, f"$.definitions.{name}")
    return build(canonical(doc["root"], "$.root"), canon_defs, domain)


def build(root: Kind, defs: Defs, domain: str | None) -> tuple[Kind, Defs, str | None]:
    """Canonical parts checked as a whole; only the definitions the root
    reaches are kept (spec 5.3)."""
    if domain is not None:
        _nonempty(domain, "$.domain")
    check(root, defs, "$.root")
    for name, k in defs.items():
        check(k, defs, f"$.definitions.{name}")
    reached: set[str] = set()
    reach(root, defs, reached)
    return root, {n: k for n, k in sorted(defs.items()) if n in reached}, domain


def reach(k: Kind, defs: Defs, reached: set[str]) -> None:
    """Adds the names of the definitions ``k`` reaches to ``reached``."""
    if _str(k):
        return
    ((name, v),) = k.items()
    if name == "ref":
        if v not in reached:
            reached.add(v)
            if v in defs:
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


# --- Checks that need the whole identity (spec 5.9) -----------------------------------


def check(k: Kind, defs: Defs, path: str) -> None:
    name = kind_name(k, path)
    if _str(k):
        return
    v = k[name]
    p = f"{path}.{name}"
    if name == "ref":
        if v not in defs:
            raise InvalidIdentity(f"no definition is named {v!r}", p)
        claims(k, defs, path)  # finds cycles without structure
    elif name in ("list", "set"):
        check(v, defs, p)
    elif name == "tuple":
        for i, x in enumerate(v):
            check(x, defs, f"{p}[{i}]")
    elif name == "map":
        check(v["key"], defs, f"{p}.key")
        check(v["value"], defs, f"{p}.value")
        accepting = [
            a for a in alternatives(v["key"], defs) if "string" in _all(claims(a, defs, p))
        ]
        if not accepting:
            raise InvalidIdentity("a map's key kind accepts strings", f"{p}.key")
        if len(accepting) > 1:
            raise InvalidIdentity(
                "a map's key union has two alternatives that accept strings", f"{p}.key"
            )
    elif name == "object":
        for fname, f in v["fields"].items():
            fp = f"{p}.fields.{fname}"
            check(f["kind"], defs, f"{fp}.kind")
            for option, kinds in (
                ("quantize", {"integer", "instant", "duration"}),
                ("decimals", {"number"}),
            ):
                if option in f and quantized_kind(f["kind"], defs, fp) not in kinds:
                    raise InvalidIdentity(f"{option} does not apply to the field's kind", fp)
    elif name == "union":
        for i, a in enumerate(v):
            check(a, defs, f"{p}[{i}]")
        union_claims(v, defs, p)
    elif name == "tagged":
        for vname, var in v["variants"].items():
            if "kind" in var:
                check(var["kind"], defs, f"{p}.variants.{vname}.kind")
        if v.get("style") == "internal":
            tag = v["tag"]
            for vname, var in v["variants"].items():
                if "kind" not in var:
                    continue
                vp = f"{p}.variants.{vname}"
                obj = resolve(var["kind"], defs, vp)
                if not (type(obj) is dict and "object" in obj):
                    raise InvalidIdentity("an internally tagged variant's kind is an object", vp)
                o = obj["object"]
                if "order" in o:
                    raise InvalidIdentity("an internally tagged variant's object has no order", vp)
                for fname, f in o["fields"].items():
                    if tag in (fname, f.get("rename", fname)):
                        raise InvalidIdentity(
                            f"the variant has a field named like the tag {tag!r}", vp
                        )


def quantized_kind(k: Kind, defs: Defs, path: str) -> str | None:
    """The name of the kind a quantization applies to: the field's kind, or
    the one alternative besides null of a union; references followed."""
    k = resolve(k, defs, path)
    if type(k) is dict and "union" in k:
        others = [a for a in k["union"] if resolve(a, defs, path) != "null"]
        if len(others) != 1:
            return None
        k = resolve(others[0], defs, path)
    return kind_name(k, path)


def resolve(k: Kind, defs: Defs, path: str) -> Kind:
    """A kind with references followed."""
    seen: list[str] = []
    while type(k) is dict and "ref" in k:
        name = k["ref"]
        if name in seen:
            raise InvalidIdentity(f"a cycle of references through {name!r}", path)
        if name not in defs:
            raise InvalidIdentity(f"no definition is named {name!r}", path)
        seen.append(name)
        k = defs[name]
    return k


def alternatives(k: Kind, defs: Defs) -> list[Kind]:
    """A kind's union alternatives, or the kind itself."""
    r = resolve(k, defs, "$")
    return list(r["union"]) if type(r) is dict and "union" in r else [k]


Claims = tuple[frozenset[str], frozenset[str]]

_TABLE: Final[dict[str, Claims]] = {
    "null": (frozenset({"null"}), frozenset()),
    "bool": (frozenset({"boolean"}), frozenset({"string"})),
    "integer": (frozenset({"number"}), frozenset({"string"})),
    "number": (frozenset({"number"}), frozenset({"string"})),
    "string": (frozenset({"string"}), frozenset()),
    "uuid": (frozenset({"string"}), frozenset()),
    "path": (frozenset({"string"}), frozenset({"object"})),
    "any": (SOURCE_TYPES, frozenset()),
    "list": (frozenset({"array"}), frozenset()),
    "set": (frozenset({"array", "set"}), frozenset()),
    "tuple": (frozenset({"array"}), frozenset()),
    "map": (frozenset({"object"}), frozenset()),
    "object": (frozenset({"object"}), frozenset()),
    "bytes": (frozenset({"bytes"}), frozenset({"array", "string"})),
}


def _all(c: Claims) -> frozenset[str]:
    return c[0] | c[1]


def claims(k: Kind, defs: Defs, path: str, visiting: tuple[str, ...] = ()) -> Claims:
    """The source types a kind accepts: natively, and by conversion."""
    name = kind_name(k, path)
    if name == "ref":
        ref = k["ref"]
        if ref in visiting:
            raise InvalidIdentity(f"a cycle of references through {ref!r} without structure", path)
        if ref not in defs:
            raise InvalidIdentity(f"no definition is named {ref!r}", path)
        return claims(defs[ref], defs, path, (*visiting, ref))
    if name == "union":
        return union_claims(k["union"], defs, path, visiting)
    if name in _TABLE:
        return _TABLE[name]
    if name in ("instant", "duration"):
        conv = {"string", "object"} | ({"number"} if "unit" in k[name] else set())
        return frozenset({name}), frozenset(conv)
    # tagged
    t = k["tagged"]
    if t.get("style", "external") != "external":
        return frozenset({"object"}), frozenset()
    native = {"object" if "kind" in var else "string" for var in t["variants"].values()}
    return frozenset(native), frozenset()


def union_claims(
    alts: list[Kind], defs: Defs, path: str, visiting: tuple[str, ...] = ()
) -> Claims:
    """A union's claims, checking that it is valid (spec 5.7)."""
    native: set[str] = set()
    conv: set[str] = set()
    objs = object_alternatives(alts, defs, path, visiting)
    for a in alts:
        n, c = claims(a, defs, path, visiting)
        overlap = native & n
        if overlap == {"object"} and objs is not None:
            overlap = set()  # told apart by their keys; checked below
        if overlap:
            raise InvalidIdentity(
                f"two union alternatives accept {sorted(overlap)} natively", path
            )
        if conv & c:
            raise InvalidIdentity(
                f"two union alternatives accept {sorted(conv & c)} by conversion", path
            )
        native |= n
        conv |= c
    if objs is not None and len(objs) > 1:
        for i, a in enumerate(objs):
            for b in objs[i + 1 :]:
                if required(a, defs, path) <= set(b["fields"]) and required(b, defs, path) <= set(
                    a["fields"]
                ):
                    raise InvalidIdentity(
                        "two object alternatives of a union could both fit one object: "
                        f"{sorted(a['fields'])} and {sorted(b['fields'])}",
                        path,
                    )
    return frozenset(native), frozenset(conv)


def object_alternatives(
    alts: list[Kind], defs: Defs, path: str, visiting: tuple[str, ...] = ()
) -> list[dict[str, Any]] | None:
    """The object kinds among a union's alternatives that accept objects
    natively, if they are all object kinds (or references to them)."""
    objs = []
    for a in alts:
        r = resolve(a, defs, path)
        if type(r) is dict and "object" in r:
            objs.append(r["object"])
        elif "object" in claims(a, defs, path, visiting)[0]:
            return None
    return objs


def required(o: dict[str, Any], defs: Defs, path: str) -> set[str]:
    """The fields of an object kind whose kinds do not accept null."""
    return {n for n, f in o["fields"].items() if "null" not in claims(f["kind"], defs, path)[0]}
