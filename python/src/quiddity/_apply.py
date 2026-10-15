"""Applying an identity to a source value (spec 5.4 to 5.8).

Sources are data-model values (``_model``), as ``_value.from_python``
reads Python values: an object is a ``Map``, whose text keys are its
members."""

from __future__ import annotations

import math
from fractions import Fraction
from typing import Any

from . import _text
from ._errors import QuiddityError
from ._load import UNITS, Claims, Defs, Kind, claims, kind_name, resolve
from ._model import Duration, Instant, Map, Set, is_empty
from ._quantize import decimals, quantize


def source_type(s: Any) -> str:
    t = type(s)
    if s is None:
        return "null"
    if t is bool:
        return "boolean"
    if t is int or t is float:
        return "number"
    if t is str:
        return "string"
    if t is bytes:
        return "bytes"
    if t is list:
        return "array"
    if t is Map:
        return "object"
    if t is Set:
        return "set"
    if t is Instant:
        return "instant"
    if t is Duration:
        return "duration"
    raise TypeError(f"not a source value: {s!r}")


def members(s: Map) -> dict[str, Any]:
    """A source object's members, by name: its entries with text keys."""
    return {k: v for k, v in s.entries if type(k) is str}


class Applier:
    """Reads source values with the kinds of one identity."""

    def __init__(self, defs: Defs) -> None:
        self.defs = defs
        self._claims: dict[int, Claims] = {}
        self._required: dict[int, set[str]] = {}

    def claims(self, k: Kind) -> Claims:
        key = id(k)
        found = self._claims.get(key)
        if found is None:
            found = claims(k, self.defs, "$")
            self._claims[key] = found
        return found

    def resolve(self, k: Kind) -> Kind:
        return resolve(k, self.defs, "$")

    def required(self, o: dict[str, Any]) -> set[str]:
        key = id(o)
        found = self._required.get(key)
        if found is None:
            found = {n for n, f in o["fields"].items() if "null" not in self.claims(f["kind"])[0]}
            self._required[key] = found
        return found

    def read(self, k: Kind, s: Any, path: str) -> Any:
        """The data-model value of source value ``s`` read with kind ``k``."""
        while type(k) is dict and "ref" in k:
            k = self.defs[k["ref"]]
        t = source_type(s)
        if type(k) is dict and "union" in k:
            return self.read(self._choose(k["union"], s, t, path), s, path)
        name = kind_name(k, path)
        native, conv = self.claims(k)
        if t not in native and t not in conv:
            raise QuiddityError(f"{name} does not accept {_a(t)}", path)
        try:
            return self._read(name, k, s, t, path)
        except QuiddityError as e:
            if e.path == "$" and path != "$":
                raise QuiddityError(e.message, path) from None
            raise

    def _choose(self, alts: list[Kind], s: Any, t: str, path: str) -> Kind:
        """The union alternative that reads ``s`` (spec 5.7)."""
        objects = [a for a in alts if _is_object(self.resolve(a))]
        if t == "object" and len(objects) > 1:
            names = set(members(s))
            for a in objects:
                o = self.resolve(a)["object"]
                if names <= set(o["fields"]) and self.required(o) <= names:
                    return a
            for a in alts:
                if "object" in self.claims(a)[1]:
                    return a
            raise QuiddityError("the object fits none of the union's object alternatives", path)
        for a in alts:
            if t in self.claims(a)[0]:
                return a
        for a in alts:
            if t in self.claims(a)[1]:
                return a
        raise QuiddityError(f"no alternative of the union accepts {_a(t)}", path)

    def _read(self, name: str, k: Kind, s: Any, t: str, path: str) -> Any:
        if name == "any":
            return s
        if name == "null":
            return None
        if name == "bool":
            return s if t == "boolean" else _text.boolean(s)
        if name == "integer":
            if t == "number":
                if type(s) is not int:
                    raise QuiddityError(f"not an integer: {s!r}")
                return s
            return _text.integer(s)
        if name == "number":
            return s if t == "number" else _text.number(s)
        if name == "string":
            return s
        if name == "uuid":
            return _text.uuid(s)
        if name == "path":
            return s if t == "string" else _path(s)
        if name == "bytes":
            if t == "bytes":
                return s
            if t == "array":
                if not all(type(b) is int and 0 <= b <= 255 for b in s):
                    raise QuiddityError("bytes are integers from 0 to 255")
                return bytes(s)
            if k["bytes"].get("text") == "hex":
                return _text.hex_bytes(s)
            return _text.base64_bytes(s)
        if name in ("instant", "duration"):
            ns = _time(s, t, k[name].get("unit"), name)
            return Instant(ns) if name == "instant" else Duration(ns)
        if name in ("list", "set"):
            source = s.items if t == "set" else s
            items = [self.read(k[name], x, f"{path}[{i}]") for i, x in enumerate(source)]
            return items if name == "list" else Set(items)
        if name == "tuple":
            kinds = k["tuple"]
            if len(s) != len(kinds):
                raise QuiddityError(f"expected {len(kinds)} items, found {len(s)}")
            return [
                self.read(kk, x, f"{path}[{i}]")
                for i, (kk, x) in enumerate(zip(kinds, s, strict=True))
            ]
        if name == "map":
            m = k["map"]
            entries = []
            for key, x in s.entries:
                p = f"{path}.{key}" if type(key) is str else f"{path}[{key!r}]"
                entries.append((self.read(m["key"], key, p), self.read(m["value"], x, p)))
            return Map(entries)
        if name == "object":
            return self._object(k["object"], members(s), path)
        return self._tagged(k["tagged"], s, path)

    def _object(self, o: dict[str, Any], ms: dict[str, Any], path: str) -> Any:
        values = []
        for fname, f in o["fields"].items():
            fp = f"{path}.{fname}"
            v = self.read(f["kind"], ms.get(fname), fp)
            try:
                if "quantize" in f:
                    v = quantize(v, f["quantize"])
                elif "decimals" in f:
                    v = decimals(v, f["decimals"])
            except QuiddityError as e:
                raise QuiddityError(e.message, fp) from None
            omit = f.get("omit", ())
            if ("none" in omit and v is None) or ("empty" in omit and is_empty(v)):
                continue
            values.append((fname, f.get("rename", fname), v))
        if "order" in o:
            by_name = {n: v for n, _, v in values}
            return [by_name[n] for n in o["order"]]
        return Map([(key, v) for _, key, v in values])

    def _tagged(self, t: dict[str, Any], s: Any, path: str) -> Any:
        style = t.get("style", "external")
        int_mode = t.get("encode_as") == "int"
        variants = t["variants"]

        def label(vname: str) -> Any:
            var = variants[vname]
            return var["discriminant"] if int_mode else var.get("rename", vname)

        def variant(vname: Any) -> dict[str, Any]:
            if type(vname) is not str or vname not in variants:
                raise QuiddityError(f"unknown variant {vname!r}", path)
            return dict(variants[vname])

        if style == "external":
            if type(s) is str:
                if "kind" in variant(s):
                    raise QuiddityError(f"the variant {s!r} has a value", path)
                return label(s)
            if type(s) is not Map or len(s.entries) != 1:
                raise QuiddityError(
                    "an externally tagged value is a string or an object with one member", path
                )
            ((vname, x),) = s.entries
            var = variant(vname)
            if "kind" not in var:
                raise QuiddityError(f"the unit variant {vname!r} has a value", path)
            v = self.read(var["kind"], x, f"{path}.{vname}")
            return [label(vname), v] if int_mode else Map([(label(vname), v)])
        tag = t["tag"]
        ms = members(s) if type(s) is Map else {}
        if tag not in ms:
            raise QuiddityError(f"expected an object with the member {tag!r}", path)
        vname = ms[tag]
        var = variant(vname)
        tag_entry = (tag, label(vname))
        if "kind" not in var:
            return Map([tag_entry])
        if style == "internal":
            o = self.resolve(var["kind"])["object"]
            rest = {n: v for n, v in ms.items() if n != tag}
            result = self._object(o, rest, path)
            return Map([*result.entries, tag_entry])
        content = t["content"]
        if content not in ms:
            raise QuiddityError(f"expected the member {content!r}", path)
        return Map(
            [tag_entry, (content, self.read(var["kind"], ms[content], f"{path}.{content}"))]
        )


def _is_object(k: Kind) -> bool:
    return type(k) is dict and "object" in k


def _a(t: str) -> str:
    return ("an " if t[0] in "aeiou" else "a ") + t


def _path(s: Map) -> bytes:
    ms = members(s)
    if len(s.entries) != 1 or len(ms) != 1 or next(iter(ms)) not in ("Unix", "Windows"):
        raise QuiddityError("a path object has exactly one member, Unix or Windows")
    ((platform, units),) = ms.items()
    limit = 255 if platform == "Unix" else 65535
    if type(units) is not list or not all(type(u) is int and 0 <= u <= limit for u in units):
        raise QuiddityError(f"{platform} is an array of integers from 0 to {limit}")
    return bytes(units) if platform == "Unix" else _text.windows_path(units)


def _exact_nanos(value: Fraction, unit: str, shown: Any) -> int:
    ns = value * UNITS[unit]
    if ns.denominator != 1:
        raise QuiddityError(f"{shown} {unit} is not a whole number of nanoseconds")
    return int(ns)


def _time(s: Any, t: str, unit: str | None, kind: str) -> int:
    """An instant or a duration (spec 5.5), as integer nanoseconds."""
    if t == kind:
        return int(s.ns)
    if t == "number":
        if unit is None:
            raise QuiddityError(f"a number read as {_a(kind)} needs a declared unit")
        if type(s) is float and not math.isfinite(s):
            raise QuiddityError(f"{s!r} is not a finite number")
        return _exact_nanos(Fraction(s), unit, s)
    if t == "string":
        text = _text.trim(s)
        if unit is not None:
            f = _text.decimal(text)
            if f is not None:
                return _exact_nanos(f, unit, s)
        return _text.instant(text) if kind == "instant" else _text.duration(text)
    # object
    names = ("secs_since_epoch", "nanos_since_epoch") if kind == "instant" else ("secs", "nanos")
    ms = members(s)
    if set(ms) != set(names) or len(s.entries) != 2:
        raise QuiddityError(f"expected exactly the members {names[0]!r} and {names[1]!r}")
    secs, nanos = ms[names[0]], ms[names[1]]
    if type(secs) is not int or type(nanos) is not int:
        raise QuiddityError(f"{names[0]} and {names[1]} are integers")
    if secs < 0 or not 0 <= nanos < 10**9:
        raise QuiddityError(f"{names[0]} and {names[1]} out of range")
    return secs * 10**9 + nanos
