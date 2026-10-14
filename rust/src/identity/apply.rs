//! Applying an identity to a JSON value (spec 5.4 to 5.8).

use alloc::borrow::ToOwned;
use alloc::collections::BTreeMap;
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use serde_json::{Map, Number, Value};

use super::load::{Claims, Types, claims, resolve};
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

pub(crate) fn apply(id: &Identity, value: &Value) -> Result<Val> {
    let cx = Cx {
        defs: &id.definitions,
    };
    cx.read(&id.root, value, "$")
}

fn source_type(v: &Value) -> Types {
    match v {
        Value::Null => Types::NULL,
        Value::Bool(_) => Types::BOOLEAN,
        Value::Number(_) => Types::NUMBER,
        Value::String(_) => Types::STRING,
        Value::Array(_) => Types::ARRAY,
        Value::Object(_) => Types::OBJECT,
    }
}

fn type_name(v: &Value) -> &'static str {
    match v {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
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
    }
}

/// A JSON number as the data-model value spec 2.7 gives it.
fn number(n: &Number) -> Val {
    if let Some(i) = n.as_i64() {
        Val::int(i.into())
    } else if let Some(u) = n.as_u64() {
        Val::int(u.into())
    } else {
        Val::Float(n.as_f64().unwrap_or(f64::NAN))
    }
}

/// A JSON number as an integer, if its value is one.
fn integer(n: &Number) -> Option<Vec<u8>> {
    if let Some(i) = n.as_i64() {
        return Some(i128::from(i).to_be_bytes().to_vec());
    }
    if let Some(u) = n.as_u64() {
        return Some(i128::from(u).to_be_bytes().to_vec());
    }
    let mut buf = [0u8; crate::number::FLOAT_INT_BUF];
    let integral = crate::number::Integral::of(n.as_f64()?)?;
    Some(integral.twos_complement(&mut buf).to_vec())
}

impl Cx<'_> {
    fn claims(&self, k: &Kind) -> Claims {
        // The identity is valid, so this cannot fail.
        claims(k, self.defs, &mut Vec::new()).expect("a valid identity")
    }

    fn read(&self, k: &Kind, v: &Value, path: &str) -> Result<Val> {
        let k = resolve(k, self.defs);
        if let Kind::Union(alts) = k {
            let t = source_type(v);
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
            (Kind::Bool, Value::Bool(b)) => Ok(Val::Bool(*b)),
            (Kind::Bool, Value::String(s)) => text::bool(s)
                .map(Val::Bool)
                .ok_or_else(|| err(path, format!("not a boolean: {s:?}"))),
            (Kind::Integer, Value::Number(n)) => integer(n)
                .map(Val::Int)
                .ok_or_else(|| err(path, format!("not an integer: {n}"))),
            (Kind::Integer, Value::String(s)) => Decimal::parse(text::trim(s))
                .ok_or(())
                .and_then(|d| d.integer(0))
                .ok()
                .flatten()
                .map(Val::Int)
                .ok_or_else(|| err(path, format!("not an integer: {s:?}"))),
            (Kind::Number, Value::Number(n)) => Ok(number(n)),
            (Kind::Number, Value::String(s)) => number_text(s, path),
            (Kind::String, Value::String(s)) => Ok(Val::str(s)),
            (Kind::Uuid, Value::String(s)) => text::uuid(s)
                .map(|u| Val::str(&u))
                .ok_or_else(|| err(path, format!("not a UUID: {s:?}"))),
            (Kind::Path, Value::String(s)) => Ok(Val::str(s)),
            (Kind::Path, Value::Object(o)) => path_object(o, path),
            (Kind::Bytes(_), Value::Array(items)) => items
                .iter()
                .map(|b| b.as_u64().filter(|b| *b <= 255).map(|b| b as u8))
                .collect::<Option<Vec<u8>>>()
                .map(Val::Str)
                .ok_or_else(|| err(path, "bytes are integers from 0 to 255")),
            (Kind::Bytes(o), Value::String(s)) => {
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
            (Kind::List(item), Value::Array(items)) => {
                Ok(Val::List(self.items(item, items, path)?))
            }
            (Kind::Set(item), Value::Array(items)) => Ok(Val::Set(self.items(item, items, path)?)),
            (Kind::Tuple(kinds), Value::Array(items)) => {
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
            (Kind::Map(m), Value::Object(o)) => {
                let mut entries = Vec::with_capacity(o.len());
                for (key, x) in o {
                    let p = format!("{path}.{key}");
                    let key = self.read(&m.key, &Value::String(key.clone()), &p)?;
                    entries.push((key, self.read(&m.value, x, &p)?));
                }
                Ok(Val::Map(entries))
            }
            (Kind::Object(obj), Value::Object(o)) => self.object(obj, o, None, path),
            (Kind::Tagged(t), v) => self.tagged(t, v, path),
            _ => Err(err(
                path,
                format!("{} does not accept a {}", kind_name(k), type_name(v)),
            )),
        }
    }

    fn items(&self, item: &Kind, items: &[Value], path: &str) -> Result<Vec<Val>> {
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
        o: &Map<String, Value>,
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
            let v = self.read(
                &f.kind,
                member.unwrap_or(&Value::Null),
                &format!("{path}.{name}"),
            )?;
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

    fn tagged(&self, t: &TaggedKind, v: &Value, path: &str) -> Result<Val> {
        let int = t.encode_as == Some(TagEncoding::Int);
        let label = |name: &str| {
            let var = &t.variants[name];
            match var.discriminant {
                Some(d) if int => Val::int(d),
                _ => Val::str(var.rename.as_deref().unwrap_or(name)),
            }
        };
        let variant = |name: &Value| -> Result<(&str, &super::Variant)> {
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
                Value::String(_) => {
                    let (name, var) = variant(v)?;
                    if var.kind.is_some() {
                        return Err(err(path, format!("variant {name:?} needs a value")));
                    }
                    Ok(label(name))
                }
                Value::Object(o) if o.len() == 1 => {
                    let (key, x) = o.iter().next().unwrap();
                    let (name, var) = variant(&Value::String(key.clone()))?;
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
fn as_is(v: &Value) -> Val {
    match v {
        Value::Null => Val::Null,
        Value::Bool(b) => Val::Bool(*b),
        Value::Number(n) => number(n),
        Value::String(s) => Val::str(s),
        Value::Array(items) => Val::List(items.iter().map(as_is).collect()),
        Value::Object(o) => Val::Map(o.iter().map(|(k, x)| (Val::str(k), as_is(x))).collect()),
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

fn path_object(o: &Map<String, Value>, path: &str) -> Result<Val> {
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
fn time(v: &Value, unit: Option<Unit>, instant: bool, path: &str) -> Result<i128> {
    let what = if instant { "instant" } else { "duration" };
    let scaled = |r: core::result::Result<Option<i128>, ()>| match r {
        Ok(Some(ns)) => Ok(ns),
        Ok(None) => Err(err(path, "not a whole number of nanoseconds")),
        Err(()) => Err(err(path, "out of range")),
    };
    match v {
        Value::Number(n) => {
            let unit =
                unit.ok_or_else(|| err(path, format!("a number as an {what} needs a unit")))?;
            let scale = unit.exponent();
            if let Some(i) = n.as_i64() {
                return Ok(i128::from(i) * 10i128.pow(scale));
            }
            if let Some(u) = n.as_u64() {
                return Ok(i128::from(u) * 10i128.pow(scale));
            }
            scaled(scaled_float(n.as_f64().unwrap_or(f64::NAN), scale))
        }
        Value::String(s) => {
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
        Value::Object(o) => {
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
