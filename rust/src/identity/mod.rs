//! Dynamic identities (spec section 5): which parts of a dynamic value, such
//! as a [`serde_json::Value`], are digested and how, as data that every
//! implementation of quiddity can read.
//!
//! An [`Identity`] is built from a [`Kind`], or loaded from an identity
//! document with [`Identity::from_json`]. [`Identity::digest`] applies it to
//! a value. An `Identity` is itself digested with
//! [`quiddity()`](crate::QuiddityExt::quiddity), under the identity of
//! identity documents (spec 5.10).
//!
//! Kinds nest: a field's kind can be another `Identity`, which becomes one
//! of the identity's definitions, under its [name](Identity::named) or a
//! generated one, and [`Kind::recursive`] refers to the identity being
//! built. Every `Identity` is valid (spec 5.9): the functions that build or
//! change one check it, and an operation that fails changes nothing.

use alloc::boxed::Box;
use alloc::collections::{BTreeMap, BTreeSet};
use alloc::string::String;
use alloc::vec::Vec;
use core::fmt;

use crate::{Digest, Quiddity};

mod apply;
mod compose;
#[doc(hidden)]
pub mod export;
mod load;
mod text;
mod value;

pub use export::ExportError;
pub use load::InvalidIdentity;

/// An identity (spec 5): the kind of the value being digested, the
/// definitions that its references refer to, and a domain.
///
/// An `Identity` is always valid (spec 5.9) and in canonical form (spec
/// 5.3).
#[derive(Clone, Debug, PartialEq, Eq, Quiddity)]
#[quiddity(domain = "quiddity.identity")]
pub struct Identity {
    #[quiddity(rename = "quiddity")]
    version: u32,
    #[quiddity(omit_none)]
    domain: Option<String>,
    root: Kind,
    #[quiddity(omit_empty)]
    definitions: BTreeMap<String, Kind>,
}

/// What a value is, and how it is read (spec 5.4).
#[derive(Clone, Debug, PartialEq, Eq, Quiddity)]
#[quiddity(rename_all = "snake_case")]
pub enum Kind {
    /// Null.
    Null,
    /// A boolean, or text such as `"yes"` (spec 5.5).
    Bool,
    /// An integer, or decimal text with an integer value.
    Integer,
    /// A number, decimal text, or text such as `"NaN"` or `"inf"`.
    Number,
    /// A string.
    String,
    /// A UUID in any common text form, as its canonical form.
    Uuid,
    /// A file system path or operating system string.
    Path,
    /// Any value, as it is.
    Any,
    /// Bytes: an array of integers, or base64 or hex text.
    Bytes(BytesOptions),
    /// An instant, as integer nanoseconds since the Unix epoch (spec 2.6).
    Instant(TimeOptions),
    /// A duration, as integer nanoseconds.
    Duration(TimeOptions),
    /// A list, each item of the given kind.
    List(Box<Kind>),
    /// A set, each member of the given kind.
    Set(Box<Kind>),
    /// A fixed-length array, each item of the kind in its position.
    Tuple(Vec<Kind>),
    /// A map, with keys and values of the given kinds.
    Map(MapKind),
    /// An object with named fields (spec 5.6).
    Object(ObjectKind),
    /// One of several kinds, told apart by the value's type (spec 5.7).
    Union(#[quiddity(encode_as = set)] Vec<Kind>),
    /// One of several named variants (spec 5.8).
    Tagged(TaggedKind),
    /// The kind defined under this name in the identity. References are
    /// made by nesting identities and by [`Kind::recursive`], never by
    /// hand, so that none can refer to a missing definition.
    Ref(RefName),
    /// An identity nested in a kind being built: it becomes a definition of
    /// the identity the kind is placed in, and a reference to it. It is
    /// never found inside an `Identity`.
    Identity(Box<Identity>),
    /// The identity a kind is being built for: it becomes a reference to
    /// that identity's own definition. It is never found inside an
    /// `Identity`.
    Recursive,
}

/// The name of a definition that a [`Kind::Ref`] refers to.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Quiddity)]
pub struct RefName(String);

impl RefName {
    pub(crate) fn new(name: String) -> RefName {
        RefName(name)
    }

    /// The name.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for RefName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// The options of [`Kind::Bytes`].
#[derive(Clone, Debug, Default, PartialEq, Eq, Quiddity)]
pub struct BytesOptions {
    /// How text is read: base64 (`None`, the default) or hex.
    #[quiddity(omit_none)]
    pub text: Option<BytesText>,
}

/// How [`Kind::Bytes`] reads text.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Quiddity)]
#[quiddity(rename_all = "lowercase")]
pub enum BytesText {
    /// Base64, standard or URL-safe, with or without padding.
    Base64,
    /// Hexadecimal digits.
    Hex,
}

/// The options of [`Kind::Instant`] and [`Kind::Duration`].
#[derive(Clone, Debug, Default, PartialEq, Eq, Quiddity)]
pub struct TimeOptions {
    /// The unit of a number. Without one, a number is an error.
    #[quiddity(omit_none)]
    pub unit: Option<Unit>,
}

/// A unit of time.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Quiddity)]
#[quiddity(rename_all = "lowercase")]
pub enum Unit {
    /// Seconds.
    S,
    /// Milliseconds.
    Ms,
    /// Microseconds.
    Us,
    /// Nanoseconds.
    Ns,
}

impl Unit {
    /// The number of nanoseconds in the unit, as a power of ten.
    fn exponent(self) -> u32 {
        match self {
            Unit::S => 9,
            Unit::Ms => 6,
            Unit::Us => 3,
            Unit::Ns => 0,
        }
    }
}

/// The options of [`Kind::Map`].
#[derive(Clone, Debug, PartialEq, Eq, Quiddity)]
pub struct MapKind {
    /// The kind of the keys. In JSON every key is a string, which this kind
    /// must accept.
    pub key: Box<Kind>,
    /// The kind of the values.
    pub value: Box<Kind>,
}

/// The options of [`Kind::Object`] (spec 5.6).
#[derive(Clone, Debug, Default, PartialEq, Eq, Quiddity)]
pub struct ObjectKind {
    /// The fields, by name.
    pub fields: BTreeMap<String, Field>,
    /// If given, the object is encoded as the list of its fields' values in
    /// this order, rather than as a map.
    #[quiddity(omit_none)]
    pub order: Option<Vec<String>>,
}

/// A field of an [`ObjectKind`].
#[derive(Clone, Debug, PartialEq, Eq, Quiddity)]
pub struct Field {
    /// The field's kind. A missing member is read as null.
    pub kind: Kind,
    /// The field's key in the result, if not its name.
    #[quiddity(omit_none)]
    pub rename: Option<String>,
    /// When the field is left out.
    #[quiddity(omit_empty)]
    pub omit: BTreeSet<Omit>,
}

/// A condition under which a field is left out.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Quiddity)]
#[quiddity(rename_all = "lowercase")]
pub enum Omit {
    /// When its value is null.
    None,
    /// When its value is an empty string, list, set or map.
    Empty,
}

/// The options of [`Kind::Tagged`] (spec 5.8).
#[derive(Clone, Debug, Default, PartialEq, Eq, Quiddity)]
pub struct TaggedKind {
    /// The variants, by name.
    pub variants: BTreeMap<String, Variant>,
    /// How variants are tagged: externally (`None`, the default),
    /// internally or adjacently.
    #[quiddity(omit_none)]
    pub style: Option<Style>,
    /// The member holding the variant's name, for internal and adjacent
    /// tagging.
    #[quiddity(omit_none)]
    pub tag: Option<String>,
    /// The member holding the variant's value, for adjacent tagging.
    #[quiddity(omit_none)]
    pub content: Option<String>,
    /// How variants are identified in the result: by name (`None`, the
    /// default) or by discriminant.
    #[quiddity(omit_none)]
    pub encode_as: Option<TagEncoding>,
}

/// How a tagged union's variants are tagged.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Quiddity)]
#[quiddity(rename_all = "lowercase")]
pub enum Style {
    /// `{"Variant": value}`, or `"Variant"` for a unit variant.
    External,
    /// `{"tag": "Variant", ...fields}`.
    Internal,
    /// `{"tag": "Variant", "content": value}`.
    Adjacent,
}

/// How a tagged union's variants are identified in the result.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Quiddity)]
#[quiddity(rename_all = "lowercase")]
pub enum TagEncoding {
    /// By name.
    String,
    /// By discriminant.
    Int,
}

/// A variant of a [`TaggedKind`].
#[derive(Clone, Debug, Default, PartialEq, Eq, Quiddity)]
pub struct Variant {
    /// The variant's kind; `None` for a unit variant.
    #[quiddity(omit_none)]
    pub kind: Option<Kind>,
    /// The variant's name in the result, if not its name.
    #[quiddity(omit_none)]
    pub rename: Option<String>,
    /// The variant's discriminant, with `TagEncoding::Int`.
    #[quiddity(omit_none)]
    pub discriminant: Option<i128>,
}

impl Kind {
    /// A list of `item`.
    pub fn list(item: impl Into<Kind>) -> Kind {
        Kind::List(Box::new(item.into()))
    }

    /// A set of `member`.
    pub fn set(member: impl Into<Kind>) -> Kind {
        Kind::Set(Box::new(member.into()))
    }

    /// `kind` or null.
    pub fn optional(kind: impl Into<Kind>) -> Kind {
        Kind::Union(alloc::vec![Kind::Null, kind.into()])
    }

    /// The identity this kind is placed in, for recursive types: a tree
    /// node's `children` field is `Kind::list(Kind::recursive())`.
    pub fn recursive() -> Kind {
        Kind::Recursive
    }

    /// Bytes, read from text as base64.
    pub fn bytes() -> Kind {
        Kind::Bytes(BytesOptions::default())
    }

    /// Bytes, read from text as hexadecimal digits.
    pub fn bytes_hex() -> Kind {
        Kind::Bytes(BytesOptions {
            text: Some(BytesText::Hex),
        })
    }

    /// An instant, read from RFC 3339 text or serde's `SystemTime` form.
    pub fn instant() -> Kind {
        Kind::Instant(TimeOptions::default())
    }

    /// An instant, also read from a number of `unit`s since the Unix epoch.
    pub fn instant_in(unit: Unit) -> Kind {
        Kind::Instant(TimeOptions { unit: Some(unit) })
    }

    /// A duration, read from ISO 8601 text or serde's `Duration` form.
    pub fn duration() -> Kind {
        Kind::Duration(TimeOptions::default())
    }

    /// A duration, also read from a number of `unit`s.
    pub fn duration_in(unit: Unit) -> Kind {
        Kind::Duration(TimeOptions { unit: Some(unit) })
    }

    /// A map from `key` to `value`. In JSON every key is a string, so `key`
    /// must accept strings.
    pub fn map(key: impl Into<Kind>, value: impl Into<Kind>) -> Result<Kind, InvalidIdentity> {
        compose::checked(Kind::Map(MapKind {
            key: Box::new(key.into()),
            value: Box::new(value.into()),
        }))
    }

    /// An object with the given fields, encoded as a map from their keys.
    pub fn object<N, F>(fields: impl IntoIterator<Item = (N, F)>) -> Result<Kind, InvalidIdentity>
    where
        N: Into<String>,
        F: Into<Field>,
    {
        compose::object(fields, false)
    }

    /// An object with the given fields, encoded as the list of their values
    /// in the order given. Its fields cannot be omitted.
    pub fn object_as_list<N, F>(
        fields: impl IntoIterator<Item = (N, F)>,
    ) -> Result<Kind, InvalidIdentity>
    where
        N: Into<String>,
        F: Into<Field>,
    {
        compose::object(fields, true)
    }

    /// One of several kinds, told apart by the value's type, or objects by
    /// their keys (spec 5.7).
    pub fn union<K: Into<Kind>>(
        alternatives: impl IntoIterator<Item = K>,
    ) -> Result<Kind, InvalidIdentity> {
        compose::checked(Kind::Union(
            alternatives.into_iter().map(Into::into).collect(),
        ))
    }

    /// A fixed-length array, each item of the kind in its position.
    pub fn tuple<K: Into<Kind>>(
        items: impl IntoIterator<Item = K>,
    ) -> Result<Kind, InvalidIdentity> {
        compose::checked(Kind::Tuple(items.into_iter().map(Into::into).collect()))
    }

    /// Variants written as `{"Name": value}`, or `"Name"` for a unit
    /// variant (`None`), as serde writes enums by default.
    pub fn externally_tagged<N: Into<String>>(
        variants: impl IntoIterator<Item = (N, Option<Kind>)>,
    ) -> Result<Kind, InvalidIdentity> {
        compose::tagged(variants, None, None, None)
    }

    /// Variants written as objects whose member `tag` names the variant,
    /// as serde's `#[serde(tag = "…")]` does. Each variant is a unit
    /// variant (`None`) or an object kind.
    pub fn internally_tagged<N: Into<String>>(
        tag: impl Into<String>,
        variants: impl IntoIterator<Item = (N, Option<Kind>)>,
    ) -> Result<Kind, InvalidIdentity> {
        compose::tagged(variants, Some(Style::Internal), Some(tag.into()), None)
    }

    /// Variants written as `{tag: "Name", content: value}`, as serde's
    /// `#[serde(tag = "…", content = "…")]` does.
    pub fn adjacently_tagged<N: Into<String>>(
        tag: impl Into<String>,
        content: impl Into<String>,
        variants: impl IntoIterator<Item = (N, Option<Kind>)>,
    ) -> Result<Kind, InvalidIdentity> {
        compose::tagged(
            variants,
            Some(Style::Adjacent),
            Some(tag.into()),
            Some(content.into()),
        )
    }

    /// The tagged union `self`, with each variant identified in the result
    /// by the given discriminant rather than its name.
    pub fn by_discriminant<N: AsRef<str>>(
        self,
        discriminants: impl IntoIterator<Item = (N, i128)>,
    ) -> Result<Kind, InvalidIdentity> {
        compose::by_discriminant(self, discriminants)
    }
}

impl From<Identity> for Kind {
    fn from(identity: Identity) -> Kind {
        Kind::Identity(Box::new(identity))
    }
}

impl Field {
    /// A field of the given kind, under its own name, never omitted.
    pub fn new(kind: impl Into<Kind>) -> Field {
        Field {
            kind: kind.into(),
            rename: None,
            omit: BTreeSet::new(),
        }
    }

    /// A field of `kind` or null, omitted when null: an `Option` that
    /// serde leaves out when it is `None`.
    pub fn optional(kind: impl Into<Kind>) -> Field {
        Field::new(Kind::optional(kind)).omit(Omit::None)
    }

    /// A field omitted when it is empty or missing: a collection that
    /// serde leaves out when it is empty.
    pub fn omit_empty(kind: impl Into<Kind>) -> Field {
        Field::new(Kind::optional(kind))
            .omit(Omit::None)
            .omit(Omit::Empty)
    }

    /// The field, with its key in the result changed to `key`.
    pub fn rename(mut self, key: impl Into<String>) -> Field {
        self.rename = Some(key.into());
        self
    }

    /// The field, left out when `condition` holds.
    pub fn omit(mut self, condition: Omit) -> Field {
        self.omit.insert(condition);
        self
    }
}

impl From<Kind> for Field {
    fn from(kind: Kind) -> Field {
        Field::new(kind)
    }
}

impl From<Identity> for Field {
    fn from(identity: Identity) -> Field {
        Field::new(identity)
    }
}

impl Identity {
    /// An identity whose root is `kind`. Identities nested in `kind` become
    /// its definitions.
    pub fn new(kind: impl Into<Kind>) -> Result<Identity, InvalidIdentity> {
        compose::identity(kind.into(), None)
    }

    /// The identity, named `name`: its root becomes its own definition
    /// under that name, which is how the identity appears in its document
    /// and in the documents of identities it is nested in. An identity that
    /// is not named is written in place where it is nested.
    pub fn named(self, name: impl Into<String>) -> Identity {
        compose::named(self, name.into())
    }

    /// The name of the identity's own definition, if its root is one: the
    /// name given with [`Identity::named`], a type's name for an exported
    /// identity, or `_self` for an unnamed recursive identity.
    pub fn name(&self) -> Option<&str> {
        match &self.root {
            Kind::Ref(name) => Some(name.as_str()),
            _ => None,
        }
    }

    /// The identity's root kind.
    pub fn root(&self) -> &Kind {
        &self.root
    }

    /// The identity's definitions, by name.
    pub fn definitions(&self) -> &BTreeMap<String, Kind> {
        &self.definitions
    }

    /// The definition `name`, as an identity of its own.
    pub fn definition(&self, name: &str) -> Option<Identity> {
        compose::definition(self, name)
    }

    /// The identity's domain, if any.
    pub fn domain(&self) -> Option<&str> {
        self.domain.as_deref()
    }

    /// Places digests under this identity in `domain` (spec 4.2), which
    /// must not be empty.
    pub fn set_domain(&mut self, domain: impl Into<String>) -> Result<(), InvalidIdentity> {
        let domain = domain.into();
        if domain.is_empty() {
            return Err(load::invalid("$.domain", "a domain is non-empty"));
        }
        self.domain = Some(domain);
        Ok(())
    }

    /// Removes the identity's domain.
    pub fn clear_domain(&mut self) {
        self.domain = None;
    }

    /// Replaces the root kind.
    pub fn set_root(&mut self, kind: impl Into<Kind>) -> Result<(), InvalidIdentity> {
        let mut next = compose::identity(kind.into(), self.domain.clone())?;
        if let Some(name) = self.name() {
            next = compose::named(next, String::from(name));
        }
        *self = next;
        Ok(())
    }

    /// Adds or replaces the field `name` of the root object.
    pub fn set_field(
        &mut self,
        name: impl Into<String>,
        field: impl Into<Field>,
    ) -> Result<(), InvalidIdentity> {
        let field = field.into();
        let recursive = compose::field_is_recursive(&field);
        compose::edit_object(self, None, recursive, |o, defs, me| {
            compose::set_field(o, name.into(), field, defs, me)
        })
    }

    /// Removes the field `name` of the root object, if it has one.
    pub fn remove_field(&mut self, name: &str) -> Result<(), InvalidIdentity> {
        compose::edit_object(self, None, false, |o, _, _| {
            o.fields.remove(name);
            if let Some(order) = &mut o.order {
                order.retain(|n| n != name);
            }
            Ok(())
        })
    }

    /// Adds or replaces the field `name` of the object defined as
    /// `definition`.
    pub fn set_field_in(
        &mut self,
        definition: &str,
        name: impl Into<String>,
        field: impl Into<Field>,
    ) -> Result<(), InvalidIdentity> {
        let field = field.into();
        let recursive = compose::field_is_recursive(&field);
        compose::edit_object(self, Some(definition), recursive, |o, defs, me| {
            compose::set_field(o, name.into(), field, defs, me)
        })
    }

    /// Removes the field `name` of the object defined as `definition`.
    pub fn remove_field_in(&mut self, definition: &str, name: &str) -> Result<(), InvalidIdentity> {
        compose::edit_object(self, Some(definition), false, |o, _, _| {
            o.fields.remove(name);
            if let Some(order) = &mut o.order {
                order.retain(|n| n != name);
            }
            Ok(())
        })
    }

    /// An identity from its parts, as export builds it.
    pub(crate) fn from_parts(
        root: Kind,
        definitions: BTreeMap<String, Kind>,
        domain: Option<String>,
    ) -> Result<Identity, InvalidIdentity> {
        load::build(root, definitions, domain)
    }

    /// Loads an identity document (spec 5.3).
    pub fn from_json(document: &serde_json::Value) -> Result<Identity, InvalidIdentity> {
        load::from_json(document)
    }

    /// The identity document, in canonical form.
    pub fn to_json(&self) -> serde_json::Value {
        load::to_json(self)
    }

    /// The digest of `value` under this identity (spec 5.1).
    pub fn digest(&self, value: &serde_json::Value) -> Result<Digest, ApplyError> {
        let v = apply::apply(self, value)?;
        let mut e = crate::Encoder::new();
        v.encode(&mut e);
        let structural = e.finish();
        Ok(match &self.domain {
            Some(label) => structural.in_domain(label),
            None => structural,
        })
    }

    /// The encoding of `value` under this identity.
    #[cfg(test)]
    pub(crate) fn encoding(&self, value: &serde_json::Value) -> Result<Vec<u8>, ApplyError> {
        let v = apply::apply(self, value)?;
        let mut e = crate::Encoder::buffer(Vec::new());
        v.encode(&mut e);
        Ok(e.into_buffer())
    }
}

/// Writes the identity document (spec 5.3), in canonical form, as
/// [`Identity::to_json`] does.
#[cfg(feature = "serde")]
impl serde::Serialize for Identity {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serde::Serialize::serialize(&self.to_json(), serializer)
    }
}

/// Reads an identity document and checks it, as [`Identity::from_json`]
/// does; a document that is not valid (spec 5.9) is an error.
#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for Identity {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let document = <serde_json::Value as serde::Deserialize>::deserialize(deserializer)?;
        Identity::from_json(&document).map_err(serde::de::Error::custom)
    }
}

/// A value that does not have the shape its identity describes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApplyError {
    /// Where in the value: `$` for the value itself, then `.field` and
    /// `[index]` steps.
    pub path: String,
    /// What is wrong.
    pub message: String,
}

impl fmt::Display for ApplyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "at {}: {}", self.path, self.message)
    }
}

#[cfg(feature = "std")]
impl std::error::Error for ApplyError {}
