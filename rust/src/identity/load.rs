//! Reading identities (spec 5.3), their canonical form, and their
//! validation (spec 5.9).

use alloc::borrow::ToOwned;
use alloc::boxed::Box;
use alloc::collections::{BTreeMap, BTreeSet};
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use core::fmt;

use super::{
    BytesOptions, BytesText, Field, Identity, Kind, MapKind, ObjectKind, Omit, Style, TagEncoding,
    TaggedKind, Variant,
};
use crate::{Encoder, Quiddity};

/// An identity that is not valid (spec 5.9).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InvalidIdentity {
    /// Where in the identity: `$` for the identity itself.
    pub path: String,
    /// What is wrong.
    pub message: String,
}

impl fmt::Display for InvalidIdentity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid identity at {}: {}", self.path, self.message)
    }
}

impl core::error::Error for InvalidIdentity {}

pub(crate) fn invalid(path: &str, message: impl Into<String>) -> InvalidIdentity {
    InvalidIdentity {
        path: path.to_owned(),
        message: message.into(),
    }
}

type Result<T> = core::result::Result<T, InvalidIdentity>;

// --- Reading identities --------------------------------------------------------

/// An identity as read (spec 5.3), before its canonical form is taken and
/// it is checked.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Unchecked {
    #[serde(default)]
    domain: Option<String>,
    root: Kind,
    #[serde(default)]
    definitions: BTreeMap<String, Kind>,
}

impl TryFrom<Unchecked> for Identity {
    type Error = InvalidIdentity;

    fn try_from(u: Unchecked) -> Result<Identity> {
        build(u.root, u.definitions, u.domain)
    }
}

/// What an object kind does with members it does not list (spec 5.6).
#[derive(serde::Deserialize)]
#[serde(rename_all = "lowercase")]
enum Unlisted {
    /// They are ignored, the default and the only choice.
    Skip,
}

/// An object kind as written: `unlisted` is read, and is not kept.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Object {
    fields: BTreeMap<String, Field>,
    #[serde(default)]
    #[allow(dead_code)]
    unlisted: Option<Unlisted>,
    #[serde(default)]
    order: Option<Vec<String>>,
}

impl From<Object> for ObjectKind {
    fn from(o: Object) -> ObjectKind {
        ObjectKind {
            fields: o.fields,
            order: o.order,
        }
    }
}

/// A field's `omit` conditions, none of them twice.
pub(crate) fn omit<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> core::result::Result<BTreeSet<Omit>, D::Error> {
    let list = <Vec<Omit> as serde::Deserialize>::deserialize(deserializer)?;
    let set: BTreeSet<Omit> = list.iter().copied().collect();
    if set.len() != list.len() {
        return Err(serde::de::Error::custom("an omit condition is given twice"));
    }
    Ok(set)
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
            if name.as_str().is_empty() {
                return Err(invalid(path, "a reference's name is empty"));
            }
            Kind::Ref(name)
        }
        Kind::Identity(_) | Kind::Recursive => {
            return Err(invalid(
                path,
                "a nested identity or a recursive kind outside an identity",
            ));
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
        if f.quantize.is_some() && f.decimals.is_some() {
            return Err(invalid(&fp, "a field has quantize or decimals, not both"));
        }
        if f.quantize == Some(0) {
            return Err(invalid(&fp, "quantize must be a positive integer"));
        }
        if f.decimals.is_some_and(|d| !(-300..=300).contains(&d)) {
            return Err(invalid(&fp, "decimals must be from -300 to 300"));
        }
        fields.insert(
            name,
            Field {
                kind: canonical(f.kind, &fp)?,
                rename,
                omit: f.omit,
                quantize: f.quantize.filter(|q| *q != 1),
                decimals: f.decimals,
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
    if t.tag.as_deref() == Some("") || t.content.as_deref() == Some("") {
        return Err(invalid(path, "tag and content are non-empty"));
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
    // Definitions the root does not reach are not part of the identity
    // (spec 5.3).
    let mut reached = BTreeSet::new();
    reach(&root, &defs, &mut reached);
    defs.retain(|name, _| reached.contains(name));
    Ok(Identity {
        domain,
        root,
        definitions: defs,
    })
}

/// Adds the names of the definitions `k` reaches to `reached`.
pub(crate) fn reach(k: &Kind, defs: &BTreeMap<String, Kind>, reached: &mut BTreeSet<String>) {
    match k {
        Kind::Ref(name) => {
            if reached.insert(name.as_str().to_owned()) {
                if let Some(d) = defs.get(name.as_str()) {
                    reach(d, defs, reached);
                }
            }
        }
        Kind::List(i) | Kind::Set(i) => reach(i, defs, reached),
        Kind::Tuple(items) | Kind::Union(items) => {
            items.iter().for_each(|i| reach(i, defs, reached))
        }
        Kind::Map(m) => {
            reach(&m.key, defs, reached);
            reach(&m.value, defs, reached);
        }
        Kind::Object(o) => o
            .fields
            .values()
            .for_each(|f| reach(&f.kind, defs, reached)),
        Kind::Tagged(t) => t
            .variants
            .values()
            .filter_map(|v| v.kind.as_ref())
            .for_each(|k| reach(k, defs, reached)),
        _ => {}
    }
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
        Kind::Identity(id) => claims(&id.root, &id.definitions, &mut Vec::new()),
        Kind::Recursive => Err("a recursive kind outside an identity".into()),
        Kind::Ref(name) => {
            let name = name.as_str();
            if visiting.iter().any(|v| v == name) {
                return Err(format!(
                    "a cycle of references through `{name}` without structure"
                ));
            }
            let def = defs
                .get(name)
                .ok_or_else(|| format!("reference to undefined `{name}`"))?;
            visiting.push(name.to_owned());
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
        match defs.get(name.as_str()) {
            Some(d) if steps <= defs.len() => {
                k = d;
                steps += 1;
            }
            _ => break,
        }
    }
    k
}

/// The kind a field's quantization applies to: its kind, or the one
/// alternative besides null of a union; references followed (spec 5.6).
pub(crate) fn quantized_kind<'k>(
    k: &'k Kind,
    defs: &'k BTreeMap<String, Kind>,
) -> Option<&'k Kind> {
    match resolve(k, defs) {
        Kind::Union(alts) => {
            let mut others = alts
                .iter()
                .map(|a| resolve(a, defs))
                .filter(|a| !matches!(a, Kind::Null));
            let only = others.next()?;
            others.next().is_none().then_some(only)
        }
        k => Some(k),
    }
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
        Kind::Object(o) => o.fields.iter().try_for_each(|(name, f)| {
            let fp = format!("{path}.fields.{name}");
            check(&f.kind, defs, &fp)?;
            let target = quantized_kind(&f.kind, defs);
            if f.quantize.is_some()
                && !matches!(
                    target,
                    Some(Kind::Integer | Kind::Instant(_) | Kind::Duration(_))
                )
            {
                return Err(invalid(
                    &fp,
                    "quantize applies to integers, instants and durations",
                ));
            }
            if f.decimals.is_some() && !matches!(target, Some(Kind::Number)) {
                return Err(invalid(&fp, "decimals applies to numbers"));
            }
            Ok(())
        }),
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
