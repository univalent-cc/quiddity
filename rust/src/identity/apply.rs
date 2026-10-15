//! Applying an identity to a source value (spec 5.4 to 5.8).

use alloc::borrow::ToOwned;
use alloc::collections::BTreeMap;
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use super::load::{Claims, Types, claims, object_alternatives, required, resolve};
use super::source::Src;
use super::text;
use super::value::{Decimal, Val, scaled_float};
use super::{ApplyError, BytesText, Identity, Kind, Omit, Style, TagEncoding, TaggedKind, Unit};

type Result<T> = core::result::Result<T, ApplyError>;

struct Cx<'a> {
    defs: &'a BTreeMap<String, Kind>,
}

fn err(path: &str, message: impl Into<String>) -> ApplyError {
    ApplyError {
        path: path.to_owned(),
        message: message.into(),
    }
}

pub(crate) fn apply(id: &Identity, value: &Src) -> Result<Val> {
    let cx = Cx {
        defs: &id.definitions,
    };
    cx.read(&id.root, value, "$")
}

/// The value of a missing member.
static NULL: Src = Src::Null;

fn source_type(v: &Src) -> Types {
    match v {
        Src::Null => Types::NULL,
        Src::Bool(_) => Types::BOOLEAN,
        Src::Int(_) | Src::Float(_) => Types::NUMBER,
        Src::Str(_) => Types::STRING,
        Src::Array(_) => Types::ARRAY,
        Src::Object(_) => Types::OBJECT,
    }
}

fn type_name(v: &Src) -> &'static str {
    match v {
        Src::Null => "null",
        Src::Bool(_) => "boolean",
        Src::Int(_) | Src::Float(_) => "number",
        Src::Str(_) => "string",
        Src::Array(_) => "array",
        Src::Object(_) => "object",
    }
}

fn kind_name(k: &Kind) -> &'static str {
    match k {
        Kind::Null => "null",
        Kind::Bool => "bool",
        Kind::Integer => "integer",
        Kind::Number => "number",
        Kind::String => "string",
        Kind::Uuid => "uuid",
        Kind::Path => "path",
        Kind::Any => "any",
        Kind::Bytes(_) => "bytes",
        Kind::Instant(_) => "instant",
        Kind::Duration(_) => "duration",
        Kind::List(_) => "list",
        Kind::Set(_) => "set",
        Kind::Tuple(_) => "tuple",
        Kind::Map(_) => "map",
        Kind::Object(_) => "object",
        Kind::Union(_) => "union",
        Kind::Tagged(_) => "tagged union",
        Kind::Ref(_) => "reference",
        Kind::Identity(_) => "nested identity",
        Kind::Recursive => "recursive kind",
    }
}

/// A source number as a data-model value. Only called on numbers.
fn number(n: &Src) -> Val {
    match n {
        Src::Int(bytes) => Val::Int(bytes.clone()),
        Src::Float(x) => Val::Float(*x),
        _ => unreachable!("a number"),
    }
}

/// A source number as an integer, if its value is one.
fn integer(n: &Src) -> Option<Vec<u8>> {
    match n {
        Src::Int(bytes) => Some(bytes.clone()),
        Src::Float(x) => {
            let mut buf = [0u8; crate::number::FLOAT_INT_BUF];
            let integral = crate::number::Integral::of(*x)?;
            Some(integral.twos_complement(&mut buf).to_vec())
        }
        _ => None,
    }
}

/// A source number, as text for error messages.
fn number_display(n: &Src) -> String {
    match (n, n.as_i128()) {
        (_, Some(i)) => format!("{i}"),
        (Src::Float(x), _) => format!("{x}"),
        _ => "an integer beyond 128 bits".to_owned(),
    }
}

impl Cx<'_> {
    fn claims(&self, k: &Kind) -> Claims {
        // The identity is valid, so this cannot fail.
        claims(k, self.defs, &mut Vec::new()).expect("a valid identity")
    }

    fn read(&self, k: &Kind, v: &Src, path: &str) -> Result<Val> {
        let k = resolve(k, self.defs);
        if let Kind::Union(alts) = k {
            let t = source_type(v);
            if let (Src::Object(o), Some(objects)) = (v, object_alternatives(alts, self.defs)) {
                if objects.len() > 1 {
                    // Told apart by their keys (spec 5.7).
                    for a in alts {
                        if let Kind::Object(obj) = resolve(a, self.defs) {
                            let required = required(obj, self.defs).expect("a valid identity");
                            if o.keys().all(|key| obj.fields.contains_key(key))
                                && required.iter().all(|r| o.contains_key(*r))
                            {
                                return self.read(a, v, path);
                            }
                        }
                    }
                    for a in alts {
                        if self.claims(a).conversion.contains(Types::OBJECT) {
                            return self.read(a, v, path);
                        }
                    }
                    return Err(err(
                        path,
                        "the object fits none of the union's object alternatives",
                    ));
                }
            }
            for natively in [true, false] {
                for a in alts {
                    let c = self.claims(a);
                    let accepts = if natively { c.native } else { c.conversion };
                    if accepts.contains(t) {
                        return self.read(a, v, path);
                    }
                }
            }
            return Err(err(
                path,
                format!("no alternative of the union accepts a {}", type_name(v)),
            ));
        }
        let c = self.claims(k);
        if !c.native.union(c.conversion).contains(source_type(v)) {
            return Err(err(
                path,
                format!("{} does not accept a {}", kind_name(k), type_name(v)),
            ));
        }
        match (k, v) {
            (Kind::Any, v) => Ok(as_is(v)),
            (Kind::Null, _) => Ok(Val::Null),
            (Kind::Bool, Src::Bool(b)) => Ok(Val::Bool(*b)),
            (Kind::Bool, Src::Str(s)) => text::bool(s)
                .map(Val::Bool)
                .ok_or_else(|| err(path, format!("not a boolean: {s:?}"))),
            (Kind::Integer, n @ (Src::Int(_) | Src::Float(_))) => integer(n)
                .map(Val::Int)
                .ok_or_else(|| err(path, format!("not an integer: {}", number_display(n)))),
            (Kind::Integer, Src::Str(s)) => Decimal::parse(text::trim(s))
                .ok_or(())
                .and_then(|d| d.integer(0))
                .ok()
                .flatten()
                .map(Val::Int)
                .ok_or_else(|| err(path, format!("not an integer: {s:?}"))),
            (Kind::Number, n @ (Src::Int(_) | Src::Float(_))) => Ok(number(n)),
            (Kind::Number, Src::Str(s)) => number_text(s, path),
            (Kind::String, Src::Str(s)) => Ok(Val::str(s)),
            (Kind::Uuid, Src::Str(s)) => text::uuid(s)
                .map(|u| Val::str(&u))
                .ok_or_else(|| err(path, format!("not a UUID: {s:?}"))),
            (Kind::Path, Src::Str(s)) => Ok(Val::str(s)),
            (Kind::Path, Src::Object(o)) => path_object(o, path),
            (Kind::Bytes(_), Src::Array(items)) => items
                .iter()
                .map(|b| b.as_u64().filter(|b| *b <= 255).map(|b| b as u8))
                .collect::<Option<Vec<u8>>>()
                .map(Val::Str)
                .ok_or_else(|| err(path, "bytes are integers from 0 to 255")),
            (Kind::Bytes(o), Src::Str(s)) => {
                let t = text::trim(s);
                let bytes = if o.text == Some(BytesText::Hex) {
                    text::hex(t)
                } else {
                    text::base64(t)
                };
                bytes
                    .map(Val::Str)
                    .ok_or_else(|| err(path, format!("not valid bytes text: {s:?}")))
            }
            (Kind::Instant(o) | Kind::Duration(o), v) => {
                let instant = matches!(k, Kind::Instant(_));
                time(v, o.unit, instant, path).map(Val::int)
            }
            (Kind::List(item), Src::Array(items)) => Ok(Val::List(self.items(item, items, path)?)),
            (Kind::Set(item), Src::Array(items)) => Ok(Val::Set(self.items(item, items, path)?)),
            (Kind::Tuple(kinds), Src::Array(items)) => {
                if kinds.len() != items.len() {
                    return Err(err(
                        path,
                        format!("expected {} items, found {}", kinds.len(), items.len()),
                    ));
                }
                kinds
                    .iter()
                    .zip(items)
                    .enumerate()
                    .map(|(i, (k, x))| self.read(k, x, &format!("{path}[{i}]")))
                    .collect::<Result<_>>()
                    .map(Val::List)
            }
            (Kind::Map(m), Src::Object(o)) => {
                let mut entries = Vec::with_capacity(o.len());
                for (key, x) in o {
                    let p = format!("{path}.{key}");
                    let key = self.read(&m.key, &Src::Str(key.clone()), &p)?;
                    entries.push((key, self.read(&m.value, x, &p)?));
                }
                Ok(Val::Map(entries))
            }
            (Kind::Object(obj), Src::Object(o)) => self.object(obj, o, None, path),
            (Kind::Tagged(t), v) => self.tagged(t, v, path),
            _ => Err(err(
                path,
                format!("{} does not accept a {}", kind_name(k), type_name(v)),
            )),
        }
    }

    fn items(&self, item: &Kind, items: &[Src], path: &str) -> Result<Vec<Val>> {
        items
            .iter()
            .enumerate()
            .map(|(i, x)| self.read(item, x, &format!("{path}[{i}]")))
            .collect()
    }

    /// An object kind applied to a source object; `skip` is a member to
    /// leave out (an internally tagged union's tag).
    fn object(
        &self,
        obj: &super::ObjectKind,
        o: &BTreeMap<String, Src>,
        skip: Option<&str>,
        path: &str,
    ) -> Result<Val> {
        let mut values = BTreeMap::new();
        let mut entries = Vec::new();
        for (name, f) in &obj.fields {
            let member = if Some(name.as_str()) == skip {
                None
            } else {
                o.get(name)
            };
            let v = self.read(&f.kind, member.unwrap_or(&NULL), &format!("{path}.{name}"))?;
            let v = quantize(v, f);
            if (f.omit.contains(&Omit::None) && v.is_null())
                || (f.omit.contains(&Omit::Empty) && v.is_empty())
            {
                continue;
            }
            match &obj.order {
                Some(_) => {
                    values.insert(name.as_str(), v);
                }
                None => entries.push((Val::str(f.rename.as_deref().unwrap_or(name)), v)),
            }
        }
        match &obj.order {
            Some(order) => Ok(Val::List(
                order
                    .iter()
                    .map(|n| values.remove(n.as_str()).unwrap_or(Val::Null))
                    .collect(),
            )),
            None => Ok(Val::Map(entries)),
        }
    }

    fn tagged(&self, t: &TaggedKind, v: &Src, path: &str) -> Result<Val> {
        let int = t.encode_as == Some(TagEncoding::Int);
        let label = |name: &str| {
            let var = &t.variants[name];
            match var.discriminant {
                Some(d) if int => Val::int(d),
                _ => Val::str(var.rename.as_deref().unwrap_or(name)),
            }
        };
        let variant = |name: &Src| -> Result<(&str, &super::Variant)> {
            let name = name
                .as_str()
                .ok_or_else(|| err(path, "a variant's name is a string"))?;
            t.variants
                .get_key_value(name)
                .map(|(n, var)| (n.as_str(), var))
                .ok_or_else(|| err(path, format!("unknown variant {name:?}")))
        };
        match t.style.unwrap_or(Style::External) {
            Style::External => match v {
                Src::Str(_) => {
                    let (name, var) = variant(v)?;
                    if var.kind.is_some() {
                        return Err(err(path, format!("variant {name:?} needs a value")));
                    }
                    Ok(label(name))
                }
                Src::Object(o) if o.len() == 1 => {
                    let (key, x) = o.iter().next().unwrap();
                    let (name, var) = variant(&Src::Str(key.clone()))?;
                    let Some(k) = &var.kind else {
                        return Err(err(path, format!("unit variant {name:?} has a value")));
                    };
                    let value = self.read(k, x, &format!("{path}.{key}"))?;
                    Ok(if int {
                        Val::List(alloc::vec![label(name), value])
                    } else {
                        Val::Map(alloc::vec![(label(name), value)])
                    })
                }
                _ => Err(err(
                    path,
                    "an externally tagged value is a string or an object with one member",
                )),
            },
            style => {
                let tag = t.tag.as_deref().unwrap_or("");
                let Some(o) = v.as_object() else {
                    return Err(err(path, "expected an object"));
                };
                let tag_value = o
                    .get(tag)
                    .ok_or_else(|| err(path, format!("missing the tag member {tag:?}")))?;
                let (name, var) = variant(tag_value)?;
                let tag_entry = (Val::str(tag), label(name));
                let Some(k) = &var.kind else {
                    return Ok(Val::Map(alloc::vec![tag_entry]));
                };
                if style == Style::Internal {
                    let Kind::Object(obj) = resolve(k, self.defs) else {
                        unreachable!("checked when the identity was built")
                    };
                    let Val::Map(mut entries) = self.object(obj, o, Some(tag), path)? else {
                        unreachable!("an object without order is a map")
                    };
                    entries.push(tag_entry);
                    return Ok(Val::Map(entries));
                }
                let content = t.content.as_deref().unwrap_or("");
                let x = o
                    .get(content)
                    .ok_or_else(|| err(path, format!("missing the content member {content:?}")))?;
                let value = self.read(k, x, &format!("{path}.{content}"))?;
                Ok(Val::Map(alloc::vec![tag_entry, (Val::str(content), value)]))
            }
        }
    }
}

/// A value as it is (spec 2.7).
fn as_is(v: &Src) -> Val {
    match v {
        Src::Null => Val::Null,
        Src::Bool(b) => Val::Bool(*b),
        Src::Int(_) | Src::Float(_) => number(v),
        Src::Str(s) => Val::str(s),
        Src::Array(items) => Val::List(items.iter().map(as_is).collect()),
        Src::Object(o) => Val::Map(o.iter().map(|(k, x)| (Val::str(k), as_is(x))).collect()),
    }
}

fn number_text(s: &str, path: &str) -> Result<Val> {
    if let Some(x) = text::special_number(s) {
        return Ok(Val::Float(x));
    }
    let t = text::trim(s);
    let d = Decimal::parse(t).ok_or_else(|| err(path, format!("not a number: {s:?}")))?;
    // As spec 2.7 gives the text as a JSON number.
    if d.plain_integer {
        let body = d.body;
        if let Ok(i) = body.parse::<i64>() {
            return Ok(Val::int(i.into()));
        }
        if let Ok(u) = body.parse::<u64>() {
            return Ok(Val::int(u.into()));
        }
    }
    let x: f64 = d
        .body
        .parse()
        .map_err(|_| err(path, format!("not a number: {s:?}")))?;
    Ok(Val::Float(x))
}

fn path_object(o: &BTreeMap<String, Src>, path: &str) -> Result<Val> {
    let shape = "a path object has exactly one member, Unix or Windows";
    if o.len() != 1 {
        return Err(err(path, shape));
    }
    let (platform, units) = o.iter().next().unwrap();
    let limit = match platform.as_str() {
        "Unix" => 255,
        "Windows" => 65_535,
        _ => return Err(err(path, shape)),
    };
    let units: Vec<u16> = units
        .as_array()
        .and_then(|a| {
            a.iter()
                .map(|u| u.as_u64().filter(|u| *u <= limit).map(|u| u as u16))
                .collect()
        })
        .ok_or_else(|| {
            err(
                path,
                format!("{platform} is an array of integers from 0 to {limit}"),
            )
        })?;
    Ok(Val::Str(if platform == "Unix" {
        units.iter().map(|u| *u as u8).collect()
    } else {
        let mut bytes = Vec::new();
        crate::encoder::wtf8(units, |b| bytes.extend_from_slice(b));
        bytes
    }))
}

/// An instant or a duration (spec 5.5), as integer nanoseconds.
fn time(v: &Src, unit: Option<Unit>, instant: bool, path: &str) -> Result<i128> {
    let what = if instant { "instant" } else { "duration" };
    let scaled = |r: core::result::Result<Option<i128>, ()>| match r {
        Ok(Some(ns)) => Ok(ns),
        Ok(None) => Err(err(path, "not a whole number of nanoseconds")),
        Err(()) => Err(err(path, "out of range")),
    };
    match v {
        Src::Int(_) | Src::Float(_) => {
            let unit =
                unit.ok_or_else(|| err(path, format!("a number as an {what} needs a unit")))?;
            let scale = unit.exponent();
            match v {
                Src::Float(x) => scaled(scaled_float(*x, scale)),
                _ => v
                    .as_i128()
                    .and_then(|i| i.checked_mul(10i128.pow(scale)))
                    .ok_or_else(|| err(path, "out of range")),
            }
        }
        Src::Str(s) => {
            let t = text::trim(s);
            if let (Some(unit), Some(d)) = (unit, Decimal::parse(t)) {
                let bytes = match d.integer(unit.exponent()) {
                    Ok(Some(b)) => b,
                    Ok(None) => return Err(err(path, "not a whole number of nanoseconds")),
                    Err(()) => return Err(err(path, "out of range")),
                };
                return small(&bytes).ok_or_else(|| err(path, "out of range"));
            }
            let parsed = if instant {
                text::instant(t)
            } else {
                text::duration(t)
            };
            parsed.map_err(|m| err(path, m))
        }
        Src::Object(o) => {
            let (secs, nanos) = if instant {
                ("secs_since_epoch", "nanos_since_epoch")
            } else {
                ("secs", "nanos")
            };
            if o.len() != 2 || !o.contains_key(secs) || !o.contains_key(nanos) {
                return Err(err(
                    path,
                    format!("expected exactly the members {secs} and {nanos}"),
                ));
            }
            let s = o[secs]
                .as_u64()
                .ok_or_else(|| err(path, format!("{secs} is an integer, at least 0")))?;
            let n = o[nanos]
                .as_u64()
                .filter(|n| *n < 1_000_000_000)
                .ok_or_else(|| err(path, format!("{nanos} is an integer from 0 to 999999999")))?;
            Ok(i128::from(s) * 1_000_000_000 + i128::from(n))
        }
        _ => Err(err(
            path,
            format!("{what} does not accept a {}", type_name(v)),
        )),
    }
}

/// Two's-complement bytes as an `i128`, if they fit.
fn small(bytes: &[u8]) -> Option<i128> {
    let b = crate::number::minimal(bytes);
    if b.len() > 16 {
        return None;
    }
    let fill = if b[0] & 0x80 != 0 { 0xFF } else { 0 };
    let mut buf = [fill; 16];
    buf[16 - b.len()..].copy_from_slice(b);
    Some(i128::from_be_bytes(buf))
}

/// A field's value, quantized as its `quantize` or `decimals` says (spec
/// 5.6). The identity is valid, so the value is of a kind that applies.
fn quantize(v: Val, f: &super::Field) -> Val {
    use crate::quantize::{self as q, Decimal};
    let int = |bytes: &[u8], step: u64| {
        let mut out = alloc::vec![0u8; bytes.len() + 9];
        let r = crate::number::minimal(q::int(bytes, step, &mut out)).to_vec();
        Val::Int(r)
    };
    let decimal = |d: Decimal| match d {
        Decimal::Float(x) => Val::Float(x),
        Decimal::Int { n, zeros } => {
            let mut buf = [0u8; q::INT_BUF];
            Val::Int(crate::number::minimal(q::scaled(n, zeros, &mut buf)).to_vec())
        }
    };
    match (v, f.quantize, f.decimals) {
        (Val::Int(bytes), Some(step), _) => int(&bytes, step),
        (Val::Float(x), _, Some(d)) => decimal(q::f64(x, d)),
        (Val::Int(bytes), _, Some(d)) if d < 0 => {
            // A number kind's integer is within 64 bits (spec 2.7), so a step
            // beyond 10^19 leaves 0, or one step below 0 for a negative value.
            let k = d.unsigned_abs();
            if k <= 19 {
                int(&bytes, 10u64.pow(k))
            } else if bytes.first().is_some_and(|b| b & 0x80 != 0) {
                decimal(Decimal::Int { n: -1, zeros: k })
            } else {
                Val::int(0)
            }
        }
        (v, _, _) => v,
    }
}
