//! Loading identity documents (spec 5.3), their canonical form, and their
//! validation (spec 5.9).

use alloc::borrow::ToOwned;
use alloc::boxed::Box;
use alloc::collections::{BTreeMap, BTreeSet};
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use core::fmt;

use serde_json::{Map, Value, json};

use super::{
    BytesOptions, BytesText, Field, Identity, Kind, MapKind, ObjectKind, Omit, Style, TagEncoding,
    TaggedKind, TimeOptions, Unit, Variant,
};
use crate::{Encoder, Quiddity};

/// An identity document that is not valid (spec 5.9).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InvalidIdentity {
    /// Where in the document: `$` for the document itself.
    pub path: String,
    /// What is wrong.
    pub message: String,
}

impl fmt::Display for InvalidIdentity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid identity at {}: {}", self.path, self.message)
    }
}

#[cfg(feature = "std")]
impl std::error::Error for InvalidIdentity {}

fn invalid(path: &str, message: impl Into<String>) -> InvalidIdentity {
    InvalidIdentity {
        path: path.to_owned(),
        message: message.into(),
    }
}

type Result<T> = core::result::Result<T, InvalidIdentity>;

// --- From JSON ---------------------------------------------------------------

/// An object's members, checking that every required one is present and no
/// other is.
fn members<'v>(
    v: &'v Value,
    path: &str,
    required: &[&str],
    optional: &[&str],
) -> Result<&'v Map<String, Value>> {
    let obj = v
        .as_object()
        .ok_or_else(|| invalid(path, "expected an object"))?;
    for m in required {
        if !obj.contains_key(*m) {
            return Err(invalid(path, format!("missing member `{m}`")));
        }
    }
    for m in obj.keys() {
        if !required.contains(&m.as_str()) && !optional.contains(&m.as_str()) {
            return Err(invalid(path, format!("unknown member `{m}`")));
        }
    }
    Ok(obj)
}

fn string(v: &Value, path: &str) -> Result<String> {
    match v.as_str() {
        Some(s) if !s.is_empty() => Ok(s.to_owned()),
        _ => Err(invalid(path, "expected a non-empty string")),
    }
}

/// One of a fixed set of strings.
fn choice<T: Copy>(v: &Value, path: &str, choices: &[(&str, T)]) -> Result<T> {
    let s = v.as_str().unwrap_or("");
    choices
        .iter()
        .find(|(name, _)| *name == s)
        .map(|(_, t)| *t)
        .ok_or_else(|| {
            let names: Vec<&str> = choices.iter().map(|(n, _)| *n).collect();
            invalid(path, format!("expected one of {names:?}, found {v}"))
        })
}

const UNITS: &[(&str, Unit)] = &[
    ("s", Unit::S),
    ("ms", Unit::Ms),
    ("us", Unit::Us),
    ("ns", Unit::Ns),
];

pub(crate) fn from_json(doc: &Value) -> Result<Identity> {
    let obj = members(doc, "$", &["quiddity", "root"], &["domain", "definitions"])?;
    if obj["quiddity"].as_u64() != Some(1) {
        return Err(invalid("$.quiddity", "expected 1"));
    }
    let domain = obj
        .get("domain")
        .map(|d| string(d, "$.domain"))
        .transpose()?;
    let root = kind(&obj["root"], "$.root")?;
    let mut definitions = BTreeMap::new();
    if let Some(defs) = obj.get("definitions") {
        let defs = defs
            .as_object()
            .ok_or_else(|| invalid("$.definitions", "expected an object"))?;
        for (name, k) in defs {
            if name.is_empty() {
                return Err(invalid("$.definitions", "a definition's name is empty"));
            }
            definitions.insert(name.clone(), kind(k, &format!("$.definitions.{name}"))?);
        }
    }
    build(root, definitions, domain)
}

fn kind(v: &Value, path: &str) -> Result<Kind> {
    if let Some(s) = v.as_str() {
        return Ok(match s {
            "null" => Kind::Null,
            "bool" => Kind::Bool,
            "integer" => Kind::Integer,
            "number" => Kind::Number,
            "string" => Kind::String,
            "uuid" => Kind::Uuid,
            "path" => Kind::Path,
            "any" => Kind::Any,
            _ => return Err(invalid(path, format!("unknown kind `{s}`"))),
        });
    }
    let obj = v
        .as_object()
        .filter(|o| o.len() == 1)
        .ok_or_else(|| invalid(path, "a kind is a string or an object with one member"))?;
    let (name, opts) = obj.iter().next().unwrap();
    let p = format!("{path}.{name}");
    Ok(match name.as_str() {
        "bytes" => {
            let o = members(opts, &p, &[], &["text"])?;
            let text = o
                .get("text")
                .map(|t| {
                    choice(
                        t,
                        &p,
                        &[("base64", BytesText::Base64), ("hex", BytesText::Hex)],
                    )
                })
                .transpose()?;
            Kind::Bytes(BytesOptions { text })
        }
        "instant" | "duration" => {
            let o = members(opts, &p, &[], &["unit"])?;
            let unit = o.get("unit").map(|u| choice(u, &p, UNITS)).transpose()?;
            let t = TimeOptions { unit };
            if name == "instant" {
                Kind::Instant(t)
            } else {
                Kind::Duration(t)
            }
        }
        "list" => Kind::List(Box::new(kind(opts, &p)?)),
        "set" => Kind::Set(Box::new(kind(opts, &p)?)),
        "tuple" => Kind::Tuple(kinds(opts, &p)?),
        "map" => {
            let o = members(opts, &p, &["key", "value"], &[])?;
            Kind::Map(MapKind {
                key: Box::new(kind(&o["key"], &format!("{p}.key"))?),
                value: Box::new(kind(&o["value"], &format!("{p}.value"))?),
            })
        }
        "object" => Kind::Object(object(opts, &p)?),
        "union" => Kind::Union(kinds(opts, &p)?),
        "tagged" => Kind::Tagged(tagged(opts, &p)?),
        "ref" => Kind::Ref(string(opts, &p)?),
        _ => return Err(invalid(path, format!("unknown kind `{name}`"))),
    })
}

fn kinds(v: &Value, path: &str) -> Result<Vec<Kind>> {
    let items = v
        .as_array()
        .ok_or_else(|| invalid(path, "expected an array of kinds"))?;
    items
        .iter()
        .enumerate()
        .map(|(i, k)| kind(k, &format!("{path}[{i}]")))
        .collect()
}

fn object(v: &Value, path: &str) -> Result<ObjectKind> {
    let o = members(v, path, &["fields"], &["unlisted", "order"])?;
    if let Some(u) = o.get("unlisted") {
        choice(u, &format!("{path}.unlisted"), &[("skip", ())])?;
    }
    let fields_obj = o["fields"]
        .as_object()
        .ok_or_else(|| invalid(path, "fields must be an object"))?;
    let mut fields = BTreeMap::new();
    for (name, f) in fields_obj {
        let fp = format!("{path}.fields.{name}");
        let fo = members(f, &fp, &["kind"], &["rename", "omit"])?;
        let mut field = Field::new(kind(&fo["kind"], &format!("{fp}.kind"))?);
        if let Some(r) = fo.get("rename") {
            field.rename = Some(string(r, &format!("{fp}.rename"))?);
        }
        if let Some(omit) = fo.get("omit") {
            let items = omit
                .as_array()
                .ok_or_else(|| invalid(&fp, "omit must be an array"))?;
            for c in items {
                let condition = choice(c, &fp, &[("none", Omit::None), ("empty", Omit::Empty)])?;
                if !field.omit.insert(condition) {
                    return Err(invalid(&fp, "an omit condition is given twice"));
                }
            }
        }
        fields.insert(name.clone(), field);
    }
    let order = match o.get("order") {
        None => None,
        Some(order) => {
            let items = order
                .as_array()
                .ok_or_else(|| invalid(path, "order must be an array"))?;
            Some(
                items
                    .iter()
                    .map(|n| string(n, &format!("{path}.order")))
                    .collect::<Result<Vec<_>>>()?,
            )
        }
    };
    Ok(ObjectKind { fields, order })
}

fn tagged(v: &Value, path: &str) -> Result<TaggedKind> {
    let o = members(
        v,
        path,
        &["variants"],
        &["style", "tag", "content", "encode_as"],
    )?;
    let style = o
        .get("style")
        .map(|s| {
            choice(
                s,
                path,
                &[
                    ("external", Style::External),
                    ("internal", Style::Internal),
                    ("adjacent", Style::Adjacent),
                ],
            )
        })
        .transpose()?;
    let encode_as = o
        .get("encode_as")
        .map(|s| {
            choice(
                s,
                path,
                &[("string", TagEncoding::String), ("int", TagEncoding::Int)],
            )
        })
        .transpose()?;
    let tag = o.get("tag").map(|t| string(t, path)).transpose()?;
    let content = o.get("content").map(|t| string(t, path)).transpose()?;
    let vs = o["variants"]
        .as_object()
        .ok_or_else(|| invalid(path, "variants must be an object"))?;
    let mut variants = BTreeMap::new();
    for (name, var) in vs {
        let vp = format!("{path}.variants.{name}");
        let vo = members(var, &vp, &[], &["kind", "rename", "discriminant"])?;
        let discriminant = match vo.get("discriminant") {
            None => None,
            Some(d) => Some(
                d.as_i64()
                    .map(i128::from)
                    .or_else(|| d.as_u64().map(i128::from))
                    .ok_or_else(|| invalid(&vp, "a discriminant is an integer"))?,
            ),
        };
        variants.insert(
            name.clone(),
            Variant {
                kind: vo.get("kind").map(|k| kind(k, &vp)).transpose()?,
                rename: vo.get("rename").map(|r| string(r, &vp)).transpose()?,
                discriminant,
            },
        );
    }
    Ok(TaggedKind {
        variants,
        style,
        tag,
        content,
        encode_as,
    })
}

// --- Canonical form and validation -------------------------------------------

/// The encoding of a kind, by which equal kinds are recognized and union
/// alternatives are put in a fixed order.
fn encoding(k: &Kind) -> Vec<u8> {
    let mut e = Encoder::buffer(Vec::new());
    k.encode(&mut e);
    e.into_buffer()
}

/// The kind in canonical form (spec 5.3), with syntax checked.
fn canonical(k: Kind, path: &str) -> Result<Kind> {
    Ok(match k {
        Kind::Bytes(BytesOptions { text }) => Kind::Bytes(BytesOptions {
            text: text.filter(|t| *t != BytesText::Base64),
        }),
        Kind::List(item) => Kind::List(Box::new(canonical(*item, path)?)),
        Kind::Set(item) => Kind::Set(Box::new(canonical(*item, path)?)),
        Kind::Tuple(items) => {
            if items.is_empty() {
                return Err(invalid(path, "a tuple needs at least one kind"));
            }
            Kind::Tuple(
                items
                    .into_iter()
                    .map(|k| canonical(k, path))
                    .collect::<Result<_>>()?,
            )
        }
        Kind::Map(MapKind { key, value }) => Kind::Map(MapKind {
            key: Box::new(canonical(*key, path)?),
            value: Box::new(canonical(*value, path)?),
        }),
        Kind::Object(o) => Kind::Object(canonical_object(o, path)?),
        Kind::Union(alts) => {
            if alts.is_empty() {
                return Err(invalid(path, "a union needs at least one kind"));
            }
            // Flatten nested unions, and merge equal alternatives (spec 5.7).
            let mut merged = BTreeMap::new();
            for a in alts {
                match canonical(a, path)? {
                    Kind::Union(inner) => {
                        for k in inner {
                            merged.insert(encoding(&k), k);
                        }
                    }
                    k => {
                        merged.insert(encoding(&k), k);
                    }
                }
            }
            let mut alts: Vec<Kind> = merged.into_values().collect();
            if alts.len() == 1 {
                alts.pop().unwrap()
            } else {
                Kind::Union(alts)
            }
        }
        Kind::Tagged(t) => Kind::Tagged(canonical_tagged(t, path)?),
        Kind::Ref(name) => {
            if name.is_empty() {
                return Err(invalid(path, "a reference's name is empty"));
            }
            Kind::Ref(name)
        }
        k @ (Kind::Null
        | Kind::Bool
        | Kind::Integer
        | Kind::Number
        | Kind::String
        | Kind::Uuid
        | Kind::Path
        | Kind::Any
        | Kind::Instant(_)
        | Kind::Duration(_)) => k,
    })
}

fn canonical_object(o: ObjectKind, path: &str) -> Result<ObjectKind> {
    let mut keys = BTreeSet::new();
    let mut fields = BTreeMap::new();
    for (name, f) in o.fields {
        let fp = format!("{path}.fields.{name}");
        if f.rename.as_deref() == Some("") {
            return Err(invalid(&fp, "rename is empty"));
        }
        let rename = f.rename.filter(|r| *r != name);
        let key = rename.clone().unwrap_or_else(|| name.clone());
        if !keys.insert(key.clone()) {
            return Err(invalid(&fp, format!("two fields have the key `{key}`")));
        }
        fields.insert(
            name,
            Field {
                kind: canonical(f.kind, &fp)?,
                rename,
                omit: f.omit,
            },
        );
    }
    if let Some(order) = &o.order {
        let named: BTreeSet<&String> = order.iter().collect();
        if named.len() != order.len()
            || named.len() != fields.len()
            || !named.iter().all(|n| fields.contains_key(*n))
        {
            return Err(invalid(path, "order must name every field exactly once"));
        }
        if fields.values().any(|f| !f.omit.is_empty()) {
            return Err(invalid(path, "a field with omit in an object with order"));
        }
    }
    Ok(ObjectKind {
        fields,
        order: o.order,
    })
}

fn canonical_tagged(t: TaggedKind, path: &str) -> Result<TaggedKind> {
    let style = t.style.unwrap_or(Style::External);
    let int = t.encode_as == Some(TagEncoding::Int);
    match style {
        Style::External if t.tag.is_some() || t.content.is_some() => {
            return Err(invalid(
                path,
                "tag and content are not allowed with external",
            ));
        }
        Style::Internal if t.tag.is_none() => return Err(invalid(path, "internal needs tag")),
        Style::Internal if t.content.is_some() => {
            return Err(invalid(path, "content is not allowed with internal"));
        }
        Style::Adjacent if t.tag.is_none() || t.content.is_none() => {
            return Err(invalid(path, "adjacent needs tag and content"));
        }
        Style::Adjacent if t.tag == t.content => {
            return Err(invalid(path, "content must differ from tag"));
        }
        _ => {}
    }
    if t.variants.is_empty() {
        return Err(invalid(path, "a tagged union needs at least one variant"));
    }
    let mut labels = BTreeSet::new();
    let mut variants = BTreeMap::new();
    for (name, v) in t.variants {
        let vp = format!("{path}.variants.{name}");
        if int != v.discriminant.is_some() {
            return Err(invalid(
                &vp,
                "a discriminant is required with encode_as int, and allowed only with it",
            ));
        }
        if v.rename.as_deref() == Some("") {
            return Err(invalid(&vp, "rename is empty"));
        }
        if v.discriminant
            .is_some_and(|d| i64::try_from(d).is_err() && u64::try_from(d).is_err())
        {
            return Err(invalid(&vp, "a discriminant is a 64-bit integer"));
        }
        let rename = v.rename.filter(|r| *r != name);
        let label = match v.discriminant {
            Some(d) => format!("#{d}"),
            None => format!("${}", rename.as_deref().unwrap_or(&name)),
        };
        if !labels.insert(label) {
            return Err(invalid(&vp, "two variants have the same label"));
        }
        variants.insert(
            name,
            Variant {
                kind: v.kind.map(|k| canonical(k, &vp)).transpose()?,
                rename,
                discriminant: v.discriminant,
            },
        );
    }
    Ok(TaggedKind {
        variants,
        style: (style != Style::External).then_some(style),
        tag: t.tag,
        content: t.content,
        encode_as: int.then_some(TagEncoding::Int),
    })
}

pub(crate) fn build(
    root: Kind,
    definitions: BTreeMap<String, Kind>,
    domain: Option<String>,
) -> Result<Identity> {
    if domain.as_deref() == Some("") {
        return Err(invalid("$.domain", "a domain is non-empty"));
    }
    let root = canonical(root, "$.root")?;
    let mut defs = BTreeMap::new();
    for (name, k) in definitions {
        if name.is_empty() {
            return Err(invalid("$.definitions", "a definition's name is empty"));
        }
        let p = format!("$.definitions.{name}");
        defs.insert(name, canonical(k, &p)?);
    }
    check(&root, &defs, "$.root")?;
    for (name, k) in &defs {
        check(k, &defs, &format!("$.definitions.{name}"))?;
    }
    Ok(Identity {
        version: 1,
        domain,
        root,
        definitions: defs,
    })
}

// --- Claims (spec 5.7) ---------------------------------------------------------

/// The source types a kind accepts.
#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
pub(crate) struct Types(pub(crate) u16);

impl Types {
    pub(crate) const NULL: Types = Types(1);
    pub(crate) const BOOLEAN: Types = Types(2);
    pub(crate) const NUMBER: Types = Types(4);
    pub(crate) const STRING: Types = Types(8);
    pub(crate) const ARRAY: Types = Types(16);
    pub(crate) const OBJECT: Types = Types(32);
    const BYTES: Types = Types(64);
    const SET: Types = Types(128);
    const INSTANT: Types = Types(256);
    const DURATION: Types = Types(512);
    const ALL: Types = Types(1023);
    const NONE: Types = Types(0);

    pub(crate) fn contains(self, t: Types) -> bool {
        self.0 & t.0 == t.0 && t.0 != 0
    }

    fn overlaps(self, t: Types) -> bool {
        self.0 & t.0 != 0
    }

    pub(crate) fn union(self, t: Types) -> Types {
        Types(self.0 | t.0)
    }
}

/// What a kind accepts: natively, and by conversion.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Claims {
    pub(crate) native: Types,
    pub(crate) conversion: Types,
}

/// The claims of `k`. Fails on a reference to an undefined name, an
/// invalid union, or a cycle of references that passes through no
/// structure.
pub(crate) fn claims(
    k: &Kind,
    defs: &BTreeMap<String, Kind>,
    visiting: &mut Vec<String>,
) -> core::result::Result<Claims, String> {
    use Types as T;
    let c = |native, conversion| Ok(Claims { native, conversion });
    match k {
        Kind::Null => c(T::NULL, T::NONE),
        Kind::Bool => c(T::BOOLEAN, T::STRING),
        Kind::Integer | Kind::Number => c(T::NUMBER, T::STRING),
        Kind::String | Kind::Uuid => c(T::STRING, T::NONE),
        Kind::Path => c(T::STRING, T::OBJECT),
        Kind::Any => c(T::ALL, T::NONE),
        Kind::Bytes(_) => c(T::BYTES, T::ARRAY.union(T::STRING)),
        Kind::Instant(o) | Kind::Duration(o) => {
            let native = if matches!(k, Kind::Instant(_)) {
                T::INSTANT
            } else {
                T::DURATION
            };
            let mut conversion = T::STRING.union(T::OBJECT);
            if o.unit.is_some() {
                conversion = conversion.union(T::NUMBER);
            }
            c(native, conversion)
        }
        Kind::List(_) | Kind::Tuple(_) => c(T::ARRAY, T::NONE),
        Kind::Set(_) => c(T::ARRAY.union(T::SET), T::NONE),
        Kind::Map(_) | Kind::Object(_) => c(T::OBJECT, T::NONE),
        Kind::Tagged(t) => {
            if t.style.is_some_and(|s| s != Style::External) {
                return c(T::OBJECT, T::NONE);
            }
            let mut native = T::NONE;
            for v in t.variants.values() {
                native = native.union(if v.kind.is_some() {
                    T::OBJECT
                } else {
                    T::STRING
                });
            }
            c(native, T::NONE)
        }
        Kind::Ref(name) => {
            if visiting.contains(name) {
                return Err(format!(
                    "a cycle of references through `{name}` without structure"
                ));
            }
            let def = defs
                .get(name)
                .ok_or_else(|| format!("reference to undefined `{name}`"))?;
            visiting.push(name.clone());
            let r = claims(def, defs, visiting);
            visiting.pop();
            r
        }
        Kind::Union(alts) => {
            let mut all = Claims {
                native: T::NONE,
                conversion: T::NONE,
            };
            let mut objects_overlap = false;
            for a in alts {
                let ca = claims(a, defs, visiting)?;
                let mut overlap = Types(all.native.0 & ca.native.0);
                if overlap == T::OBJECT {
                    // Objects may be told apart by their keys; checked below,
                    // once the alternatives are known to have no cycles.
                    objects_overlap = true;
                    overlap = T::NONE;
                }
                if overlap != T::NONE {
                    return Err("two union alternatives accept the same type natively".into());
                }
                if all.conversion.overlaps(ca.conversion) {
                    return Err("two union alternatives accept the same type by conversion".into());
                }
                all.native = all.native.union(ca.native);
                all.conversion = all.conversion.union(ca.conversion);
            }
            if objects_overlap {
                let Some(objects) = object_alternatives(alts, defs) else {
                    return Err("two union alternatives accept objects natively, \
                                and not all are object kinds"
                        .into());
                };
                for (i, a) in objects.iter().enumerate() {
                    for b in &objects[i + 1..] {
                        if fits_keys(&required(a, defs)?, b) && fits_keys(&required(b, defs)?, a) {
                            return Err(format!(
                                "two object alternatives of a union could both fit one object: {:?} and {:?}",
                                a.fields.keys().collect::<Vec<_>>(),
                                b.fields.keys().collect::<Vec<_>>()
                            ));
                        }
                    }
                }
            }
            Ok(all)
        }
    }
}

/// The object kinds among a union's alternatives that accept objects
/// natively, if every such alternative is an object kind or a reference to
/// one (spec 5.7); `None` otherwise.
pub(crate) fn object_alternatives<'k>(
    alts: &'k [Kind],
    defs: &'k BTreeMap<String, Kind>,
) -> Option<Vec<&'k ObjectKind>> {
    let mut objects = Vec::new();
    for a in alts {
        match resolve(a, defs) {
            Kind::Object(o) => objects.push(o),
            other => {
                let accepts_objects = claims(other, defs, &mut Vec::new())
                    .is_ok_and(|c| c.native.contains(Types::OBJECT));
                if accepts_objects {
                    return None;
                }
            }
        }
    }
    Some(objects)
}

/// The fields of an object kind whose kinds do not accept null.
pub(crate) fn required<'k>(
    o: &'k ObjectKind,
    defs: &BTreeMap<String, Kind>,
) -> core::result::Result<Vec<&'k str>, String> {
    let mut out = Vec::new();
    for (name, f) in &o.fields {
        if !claims(&f.kind, defs, &mut Vec::new())?
            .native
            .contains(Types::NULL)
        {
            out.push(name.as_str());
        }
    }
    Ok(out)
}

/// Whether every name in `names` is a field of `o`.
fn fits_keys(names: &[&str], o: &ObjectKind) -> bool {
    names.iter().all(|n| o.fields.contains_key(*n))
}

/// A kind with references followed.
pub(crate) fn resolve<'k>(mut k: &'k Kind, defs: &'k BTreeMap<String, Kind>) -> &'k Kind {
    let mut steps = 0;
    while let Kind::Ref(name) = k {
        match defs.get(name) {
            Some(d) if steps <= defs.len() => {
                k = d;
                steps += 1;
            }
            _ => break,
        }
    }
    k
}

/// The checks of spec 5.9 that need the whole identity.
fn check(k: &Kind, defs: &BTreeMap<String, Kind>, path: &str) -> Result<()> {
    claims(k, defs, &mut Vec::new()).map_err(|m| invalid(path, m))?;
    match k {
        Kind::List(item) | Kind::Set(item) => check(item, defs, path),
        Kind::Tuple(items) | Kind::Union(items) => {
            items.iter().try_for_each(|i| check(i, defs, path))
        }
        Kind::Map(m) => {
            check(&m.key, defs, path)?;
            check(&m.value, defs, path)?;
            let alts: &[Kind] = match resolve(&m.key, defs) {
                Kind::Union(alts) => alts,
                k => core::slice::from_ref(k),
            };
            let mut accepting = 0;
            for a in alts {
                let c = claims(a, defs, &mut Vec::new()).map_err(|m| invalid(path, m))?;
                if c.native.union(c.conversion).contains(Types::STRING) {
                    accepting += 1;
                }
            }
            match accepting {
                0 => Err(invalid(path, "a map's key kind must accept strings")),
                1 => Ok(()),
                _ => Err(invalid(
                    path,
                    "a map's key union has two alternatives that accept strings",
                )),
            }
        }
        Kind::Object(o) => o
            .fields
            .iter()
            .try_for_each(|(name, f)| check(&f.kind, defs, &format!("{path}.fields.{name}"))),
        Kind::Tagged(t) => {
            for (name, v) in &t.variants {
                let vp = format!("{path}.variants.{name}");
                if let Some(vk) = &v.kind {
                    check(vk, defs, &vp)?;
                    if t.style == Some(Style::Internal) {
                        let Kind::Object(o) = resolve(vk, defs) else {
                            return Err(invalid(&vp, "an internally tagged variant is an object"));
                        };
                        if o.order.is_some() {
                            return Err(invalid(&vp, "an internally tagged variant has order"));
                        }
                        let tag = t.tag.as_deref().unwrap_or("");
                        if o.fields
                            .iter()
                            .any(|(n, f)| n == tag || f.rename.as_deref() == Some(tag))
                        {
                            return Err(invalid(&vp, "a variant has a field named like the tag"));
                        }
                    }
                }
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

// --- To JSON -------------------------------------------------------------------

pub(crate) fn to_json(id: &Identity) -> Value {
    let mut doc = Map::new();
    doc.insert("quiddity".into(), json!(1));
    if let Some(d) = &id.domain {
        doc.insert("domain".into(), json!(d));
    }
    doc.insert("root".into(), kind_json(&id.root));
    if !id.definitions.is_empty() {
        let defs: Map<String, Value> = id
            .definitions
            .iter()
            .map(|(n, k)| (n.clone(), kind_json(k)))
            .collect();
        doc.insert("definitions".into(), Value::Object(defs));
    }
    Value::Object(doc)
}

fn unit_name(u: Unit) -> &'static str {
    UNITS.iter().find(|(_, x)| *x == u).unwrap().0
}

fn kind_json(k: &Kind) -> Value {
    let one = |name: &str, v: Value| {
        let mut m = Map::new();
        m.insert(name.into(), v);
        Value::Object(m)
    };
    match k {
        Kind::Null => json!("null"),
        Kind::Bool => json!("bool"),
        Kind::Integer => json!("integer"),
        Kind::Number => json!("number"),
        Kind::String => json!("string"),
        Kind::Uuid => json!("uuid"),
        Kind::Path => json!("path"),
        Kind::Any => json!("any"),
        Kind::Bytes(o) => one(
            "bytes",
            match o.text {
                Some(BytesText::Hex) => json!({"text": "hex"}),
                _ => json!({}),
            },
        ),
        Kind::Instant(o) | Kind::Duration(o) => {
            let name = if matches!(k, Kind::Instant(_)) {
                "instant"
            } else {
                "duration"
            };
            one(
                name,
                match o.unit {
                    Some(u) => json!({"unit": unit_name(u)}),
                    None => json!({}),
                },
            )
        }
        Kind::List(i) => one("list", kind_json(i)),
        Kind::Set(i) => one("set", kind_json(i)),
        Kind::Tuple(items) => one("tuple", items.iter().map(kind_json).collect()),
        Kind::Map(m) => one(
            "map",
            json!({"key": kind_json(&m.key), "value": kind_json(&m.value)}),
        ),
        Kind::Object(o) => {
            let fields: Map<String, Value> = o
                .fields
                .iter()
                .map(|(n, f)| {
                    let mut fo = Map::new();
                    fo.insert("kind".into(), kind_json(&f.kind));
                    if let Some(r) = &f.rename {
                        fo.insert("rename".into(), json!(r));
                    }
                    if !f.omit.is_empty() {
                        let omit: Vec<&str> = f
                            .omit
                            .iter()
                            .map(|c| match c {
                                Omit::None => "none",
                                Omit::Empty => "empty",
                            })
                            .collect();
                        fo.insert("omit".into(), json!(omit));
                    }
                    (n.clone(), Value::Object(fo))
                })
                .collect();
            let mut obj = Map::new();
            obj.insert("fields".into(), Value::Object(fields));
            if let Some(order) = &o.order {
                obj.insert("order".into(), json!(order));
            }
            one("object", Value::Object(obj))
        }
        Kind::Union(alts) => one("union", alts.iter().map(kind_json).collect()),
        Kind::Tagged(t) => {
            let variants: Map<String, Value> = t
                .variants
                .iter()
                .map(|(n, v)| {
                    let mut vo = Map::new();
                    if let Some(k) = &v.kind {
                        vo.insert("kind".into(), kind_json(k));
                    }
                    if let Some(r) = &v.rename {
                        vo.insert("rename".into(), json!(r));
                    }
                    if let Some(d) = v.discriminant {
                        let d = match i64::try_from(d) {
                            Ok(i) => json!(i),
                            Err(_) => json!(d as u64),
                        };
                        vo.insert("discriminant".into(), d);
                    }
                    (n.clone(), Value::Object(vo))
                })
                .collect();
            let mut obj = Map::new();
            obj.insert("variants".into(), Value::Object(variants));
            if let Some(s) = t.style {
                let name = match s {
                    Style::External => "external",
                    Style::Internal => "internal",
                    Style::Adjacent => "adjacent",
                };
                obj.insert("style".into(), json!(name));
            }
            if let Some(tag) = &t.tag {
                obj.insert("tag".into(), json!(tag));
            }
            if let Some(c) = &t.content {
                obj.insert("content".into(), json!(c));
            }
            if t.encode_as == Some(TagEncoding::Int) {
                obj.insert("encode_as".into(), json!("int"));
            }
            one("tagged", Value::Object(obj))
        }
        Kind::Ref(name) => one("ref", json!(name)),
    }
}
