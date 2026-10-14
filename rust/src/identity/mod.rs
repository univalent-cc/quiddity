//! Dynamic identities (spec section 5): which parts of a dynamic value, such
//! as a [`serde_json::Value`], are digested and how, as data that every
//! implementation of quiddity can read.
//!
//! An [`Identity`] is built from a [`Kind`], or loaded from an identity
//! document with [`Identity::from_json`]. [`Identity::digest`] applies it to
//! a value. An `Identity` is itself digested with
//! [`quiddity()`](crate::QuiddityExt::quiddity), under the identity of
//! identity documents (spec 5.10).

use alloc::boxed::Box;
use alloc::collections::{BTreeMap, BTreeSet};
use alloc::string::String;
use alloc::vec::Vec;
use core::fmt;

use crate::{Digest, Quiddity};

mod apply;
mod load;
mod text;
mod value;

pub use load::InvalidIdentity;

/// An identity (spec 5): the kind of the value being digested, the named
/// kinds that `Kind::Ref` refers to, and a domain.
///
/// An `Identity` is always valid (spec 5.9) and in canonical form (spec
/// 5.3): [`Identity::new`] and [`Identity::from_json`] check and normalize
/// it. To change one, take it apart with [`Identity::into_parts`] and build
/// it again.
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
    /// The kind defined under this name in the identity.
    Ref(String),
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
    pub fn list(item: Kind) -> Kind {
        Kind::List(Box::new(item))
    }

    /// A set of `member`.
    pub fn set(member: Kind) -> Kind {
        Kind::Set(Box::new(member))
    }

    /// A map from `key` to `value`.
    pub fn map(key: Kind, value: Kind) -> Kind {
        Kind::Map(MapKind {
            key: Box::new(key),
            value: Box::new(value),
        })
    }

    /// `kind` or null.
    pub fn optional(kind: Kind) -> Kind {
        Kind::Union(alloc::vec![Kind::Null, kind])
    }

    /// A reference to the definition `name`.
    pub fn reference(name: impl Into<String>) -> Kind {
        Kind::Ref(name.into())
    }

    /// An object with the given fields, encoded as a map.
    pub fn object<N: Into<String>>(fields: impl IntoIterator<Item = (N, Field)>) -> Kind {
        Kind::Object(ObjectKind {
            fields: fields.into_iter().map(|(n, f)| (n.into(), f)).collect(),
            order: None,
        })
    }
}

impl Field {
    /// A field of the given kind, under its own name, never omitted.
    pub fn new(kind: Kind) -> Field {
        Field {
            kind,
            rename: None,
            omit: BTreeSet::new(),
        }
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

impl Identity {
    /// An identity with the given root kind, and no definitions or domain.
    pub fn new(root: Kind) -> Result<Identity, InvalidIdentity> {
        Identity::from_parts(root, BTreeMap::new(), None)
    }

    /// An identity from its parts: the root kind, the definitions that
    /// references refer to, and the domain. It is checked (spec 5.9) and
    /// put in canonical form (spec 5.3).
    pub fn from_parts(
        root: Kind,
        definitions: BTreeMap<String, Kind>,
        domain: Option<String>,
    ) -> Result<Identity, InvalidIdentity> {
        load::build(root, definitions, domain)
    }

    /// The identity's root kind.
    pub fn root(&self) -> &Kind {
        &self.root
    }

    /// The identity's definitions.
    pub fn definitions(&self) -> &BTreeMap<String, Kind> {
        &self.definitions
    }

    /// The identity's domain, if any.
    pub fn domain(&self) -> Option<&str> {
        self.domain.as_deref()
    }

    /// The root kind, the definitions and the domain, to build a changed
    /// identity from.
    pub fn into_parts(self) -> (Kind, BTreeMap<String, Kind>, Option<String>) {
        (self.root, self.definitions, self.domain)
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
