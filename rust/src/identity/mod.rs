//! Dynamic identities (spec section 5): which parts of a value, such as a
//! `serde_json::Value` or any other `Serialize` value, are digested and
//! how, as data that every implementation of quiddity can read.
//!
//! An [`Identity`] is built from a [`Kind`], or read from an identity
//! document through its `Deserialize` impl, in any serde format (with the
//! `serde_json` feature, `Identity::from_json` reads a `serde_json::Value`).
//! Its `Serialize` impl writes its document. [`Identity::digest`] applies it
//! to a value. An `Identity` is itself digested with
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

use crate::{Digest, Encoder, Quiddity};

mod apply;
mod compose;
#[doc(hidden)]
pub mod export;
mod load;
mod source;
mod text;
mod value;

pub use export::ExportError;
pub use load::InvalidIdentity;

/// An identity (spec 5): the kind of the value being digested, the
/// definitions that its references refer to, and a domain.
///
/// An `Identity` is always valid (spec 5.9) and in canonical form (spec
/// 5.3).
///
/// # Examples
///
/// ```
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// # use quiddity::identity::{Field, Identity, Kind, Omit, Unit};
/// # use serde_json::json;
/// let task = Identity::new(Kind::object([
///     ("prompt", Field::new(Kind::String)),
///     ("priority", Field::new(Kind::Integer)),
///     ("note", Field::optional(Kind::String)),
/// ])?)?;
/// // `submitted_at` is not a field of the identity, so it does not count.
/// let a = task.digest(&json!({"prompt": "p", "priority": 1, "submitted_at": "09:30"}))?;
/// let b = task.digest(&json!({"prompt": "p", "priority": 1, "submitted_at": "17:45"}))?;
/// assert_eq!(a, b);
/// # Ok(())
/// # }
/// ```
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(try_from = "load::Document")]
pub struct Identity {
    #[serde(rename = "quiddity")]
    version: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    domain: Option<String>,
    root: Kind,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    definitions: BTreeMap<String, Kind>,
}

/// An identity is digested as its document, under the identity of identity
/// documents (spec 5.10 and Appendix C).
///
/// NOTE: This impl *could* have been derived, but we don't want the dynamic identity feature to
/// require the static derive macro feature
impl Quiddity for Identity {
    const DOMAIN: Option<&'static str> = Some("quiddity.identity");

    fn encode(&self, e: &mut Encoder) {
        let mut f = e.fields();
        if !self.definitions.is_empty() {
            f.field("definitions", &self.definitions);
        }
        if let Some(v) = &self.domain {
            f.field("domain", v);
        }
        f.field("quiddity", &self.version);
        f.field("root", &self.root);
        f.finish();
    }

    fn __describe(cx: &mut export::Exporter) -> Result<Kind, ExportError> {
        Ok(cx.nest(meta_identity()))
    }
}

/// The identity of identity documents (spec Appendix C), which digests
/// every identity, as `Identity`'s `Quiddity` impl encodes it.
fn meta_identity() -> Identity {
    use alloc::borrow::ToOwned;
    let r = |name: &str| Kind::Ref(RefName::new(name.to_owned()));
    // Null or `kind`, omitted when null; `Omit::Empty` too when `empty`.
    let opt = |kind: Kind, empty: bool| {
        let f = Field::optional(kind);
        if empty { f.omit(Omit::Empty) } else { f }
    };
    fn object<const N: usize>(fields: [(&str, Field); N]) -> Kind {
        Kind::Object(ObjectKind {
            fields: fields.into_iter().map(|(n, f)| (n.to_owned(), f)).collect(),
            order: None,
        })
    }
    let map = |value: Kind| {
        Kind::Map(MapKind {
            key: Box::new(Kind::String),
            value: Box::new(value),
        })
    };
    let variant = |kind: Option<Kind>| Variant {
        kind,
        rename: None,
        discriminant: None,
    };
    let units = [
        "null", "bool", "integer", "number", "string", "uuid", "path", "any",
    ];
    let mut variants: BTreeMap<String, Variant> = units
        .iter()
        .map(|n| ((*n).to_owned(), variant(None)))
        .collect();
    for (name, kind) in [
        ("bytes", object([("text", opt(Kind::String, false))])),
        ("instant", r("Time")),
        ("duration", r("Time")),
        ("list", r("Kind")),
        ("set", r("Kind")),
        ("tuple", Kind::list(r("Kind"))),
        (
            "map",
            object([
                ("key", Field::new(r("Kind"))),
                ("value", Field::new(r("Kind"))),
            ]),
        ),
        ("object", r("Object")),
        ("union", Kind::set(r("Kind"))),
        ("tagged", r("Tagged")),
        ("ref", Kind::String),
    ] {
        variants.insert(name.to_owned(), variant(Some(kind)));
    }
    let definitions = BTreeMap::from([
        (
            "Document",
            object([
                ("quiddity", Field::new(Kind::Integer)),
                ("domain", opt(Kind::String, false)),
                ("root", Field::new(r("Kind"))),
                ("definitions", opt(map(r("Kind")), true)),
            ]),
        ),
        (
            "Kind",
            Kind::Tagged(TaggedKind {
                variants,
                ..TaggedKind::default()
            }),
        ),
        ("Time", object([("unit", opt(Kind::String, false))])),
        (
            "Object",
            object([
                ("fields", Field::new(map(r("Field")))),
                ("unlisted", opt(Kind::String, false)),
                ("order", opt(Kind::list(Kind::String), false)),
            ]),
        ),
        (
            "Field",
            object([
                ("kind", Field::new(r("Kind"))),
                ("rename", opt(Kind::String, false)),
                ("omit", opt(Kind::set(Kind::String), true)),
                ("quantize", opt(Kind::Integer, false)),
                ("decimals", opt(Kind::Integer, false)),
            ]),
        ),
        (
            "Tagged",
            object([
                ("variants", Field::new(map(r("Variant")))),
                ("style", opt(Kind::String, false)),
                ("tag", opt(Kind::String, false)),
                ("content", opt(Kind::String, false)),
                ("encode_as", opt(Kind::String, false)),
            ]),
        ),
        (
            "Variant",
            object([
                ("kind", opt(r("Kind"), false)),
                ("rename", opt(Kind::String, false)),
                ("discriminant", opt(Kind::Integer, false)),
            ]),
        ),
    ])
    .into_iter()
    .map(|(n, k)| (n.to_owned(), k))
    .collect();
    Identity::from_parts(
        r("Document"),
        definitions,
        Some("quiddity.identity".to_owned()),
    )
    .expect("the identity of identity documents is valid")
}

/// What a value is, and how it is read (spec 5.4).
///
/// # Examples
///
/// ```
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// # use quiddity::identity::{Field, Identity, Kind, Omit, Unit};
/// # use serde_json::json;
/// let point = Kind::object([("x", Kind::Integer), ("y", Kind::Integer)])?;
/// let path = Kind::list(point);
/// assert!(matches!(path, Kind::List(_)));
/// # Ok(())
/// # }
/// ```
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
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
    Union(Vec<Kind>),
    /// One of several named variants (spec 5.8).
    Tagged(TaggedKind),
    /// The kind defined under this name in the identity. References are
    /// made by nesting identities and by [`Kind::recursive`], never by
    /// hand, so that none can refer to a missing definition.
    Ref(RefName),
    /// An identity nested in a kind being built: it becomes a definition of
    /// the identity the kind is placed in, and a reference to it. It is
    /// never found inside an `Identity`.
    #[serde(skip)]
    Identity(Box<Identity>),
    /// The identity a kind is being built for: it becomes a reference to
    /// that identity's own definition. It is never found inside an
    /// `Identity`.
    #[serde(skip)]
    Recursive,
}

impl Quiddity for Kind {
    fn encode(&self, e: &mut Encoder) {
        /// A variant with content: `{name: content}`.
        fn with(e: &mut Encoder, name: &'static str, content: &dyn Fn(&mut Encoder)) {
            let mut f = e.fields();
            f.field_with(name, content);
            f.finish();
        }
        match self {
            Kind::Null => e.str("null"),
            Kind::Bool => e.str("bool"),
            Kind::Integer => e.str("integer"),
            Kind::Number => e.str("number"),
            Kind::String => e.str("string"),
            Kind::Uuid => e.str("uuid"),
            Kind::Path => e.str("path"),
            Kind::Any => e.str("any"),
            Kind::Recursive => e.str("recursive"),
            Kind::Bytes(o) => with(e, "bytes", &|e| o.encode(e)),
            Kind::Instant(o) => with(e, "instant", &|e| o.encode(e)),
            Kind::Duration(o) => with(e, "duration", &|e| o.encode(e)),
            Kind::List(k) => with(e, "list", &|e| k.encode(e)),
            Kind::Set(k) => with(e, "set", &|e| k.encode(e)),
            Kind::Tuple(items) => with(e, "tuple", &|e| items.encode(e)),
            Kind::Map(m) => with(e, "map", &|e| m.encode(e)),
            Kind::Object(o) => with(e, "object", &|e| o.encode(e)),
            Kind::Union(items) => with(e, "union", &|e| {
                let mut s = e.set();
                for k in items {
                    s.member(k);
                }
                s.finish();
            }),
            Kind::Tagged(t) => with(e, "tagged", &|e| t.encode(e)),
            Kind::Ref(r) => with(e, "ref", &|e| r.encode(e)),
            Kind::Identity(id) => with(e, "identity", &|e| id.encode(e)),
        }
    }
}

/// The name of a definition that a [`Kind::Ref`] refers to.
#[derive(
    Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
#[serde(transparent)]
pub struct RefName(String);

impl Quiddity for RefName {
    fn encode(&self, e: &mut Encoder) {
        e.str(&self.0);
    }
}

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
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BytesOptions {
    /// How text is read: base64 (`None`, the default) or hex.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<BytesText>,
}

impl Quiddity for BytesOptions {
    fn encode(&self, e: &mut Encoder) {
        let mut f = e.fields();
        if let Some(v) = &self.text {
            f.field("text", v);
        }
        f.finish();
    }
}

/// How [`Kind::Bytes`] reads text.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
#[serde(rename_all = "lowercase")]
pub enum BytesText {
    /// Base64, standard or URL-safe, with or without padding.
    Base64,
    /// Hexadecimal digits.
    Hex,
}

impl Quiddity for BytesText {
    fn encode(&self, e: &mut Encoder) {
        e.str(match self {
            BytesText::Base64 => "base64",
            BytesText::Hex => "hex",
        });
    }
}

/// The options of [`Kind::Instant`] and [`Kind::Duration`].
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TimeOptions {
    /// The unit of a number. Without one, a number is an error.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unit: Option<Unit>,
}

impl Quiddity for TimeOptions {
    fn encode(&self, e: &mut Encoder) {
        let mut f = e.fields();
        if let Some(v) = &self.unit {
            f.field("unit", v);
        }
        f.finish();
    }
}

/// A unit of time.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
#[serde(rename_all = "lowercase")]
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

impl Quiddity for Unit {
    fn encode(&self, e: &mut Encoder) {
        e.str(match self {
            Unit::S => "s",
            Unit::Ms => "ms",
            Unit::Us => "us",
            Unit::Ns => "ns",
        });
    }
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
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MapKind {
    /// The kind of the keys. In JSON every key is a string, which this kind
    /// must accept.
    pub key: Box<Kind>,
    /// The kind of the values.
    pub value: Box<Kind>,
}

impl Quiddity for MapKind {
    fn encode(&self, e: &mut Encoder) {
        let mut f = e.fields();
        f.field("key", &self.key);
        f.field("value", &self.value);
        f.finish();
    }
}

/// The options of [`Kind::Object`] (spec 5.6).
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(from = "load::Object")]
pub struct ObjectKind {
    /// The fields, by name.
    pub fields: BTreeMap<String, Field>,
    /// If given, the object is encoded as the list of its fields' values in
    /// this order, rather than as a map.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<Vec<String>>,
}

impl Quiddity for ObjectKind {
    fn encode(&self, e: &mut Encoder) {
        let mut f = e.fields();
        f.field("fields", &self.fields);
        if let Some(v) = &self.order {
            f.field("order", v);
        }
        f.finish();
    }
}

/// A field of an [`ObjectKind`].
///
/// # Examples
///
/// ```
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// # use quiddity::identity::{Field, Identity, Kind, Omit, Unit};
/// # use serde_json::json;
/// // A bare kind is a field that is always present and never omitted.
/// let id = Identity::new(Kind::object([
///     ("brief", Field::new(Kind::String)),
///     ("tags", Field::omit_empty(Kind::set(Kind::String))),
///     ("kind", Field::new(Kind::String).rename("type")),
/// ])?)?;
/// assert!(id.digest(&json!({"brief": "b", "kind": "q"})).is_ok());
/// # Ok(())
/// # }
/// ```
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Field {
    /// The field's kind. A missing member is read as null.
    pub kind: Kind,
    /// The field's key in the result, if not its name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rename: Option<String>,
    /// When the field is left out.
    #[serde(
        default,
        skip_serializing_if = "BTreeSet::is_empty",
        deserialize_with = "load::omit"
    )]
    pub omit: BTreeSet<Omit>,
    /// Quantizes the value, an integer, instant or duration, to a multiple
    /// of this (in nanoseconds for instants and durations).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quantize: Option<u64>,
    /// Quantizes the value, a number, to a multiple of 10^−decimals.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub decimals: Option<i32>,
}

impl Quiddity for Field {
    fn encode(&self, e: &mut Encoder) {
        let mut f = e.fields();
        if let Some(v) = &self.decimals {
            f.field("decimals", v);
        }
        f.field("kind", &self.kind);
        if !self.omit.is_empty() {
            f.field("omit", &self.omit);
        }
        if let Some(v) = &self.quantize {
            f.field("quantize", v);
        }
        if let Some(v) = &self.rename {
            f.field("rename", v);
        }
        f.finish();
    }
}

/// A condition under which a field is left out.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
#[serde(rename_all = "lowercase")]
pub enum Omit {
    /// When its value is null.
    None,
    /// When its value is an empty string, list, set or map.
    Empty,
}

impl Quiddity for Omit {
    fn encode(&self, e: &mut Encoder) {
        e.str(match self {
            Omit::None => "none",
            Omit::Empty => "empty",
        });
    }
}

/// The options of [`Kind::Tagged`] (spec 5.8).
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TaggedKind {
    /// The variants, by name.
    pub variants: BTreeMap<String, Variant>,
    /// How variants are tagged: externally (`None`, the default),
    /// internally or adjacently.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub style: Option<Style>,
    /// The member holding the variant's name, for internal and adjacent
    /// tagging.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tag: Option<String>,
    /// The member holding the variant's value, for adjacent tagging.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    /// How variants are identified in the result: by name (`None`, the
    /// default) or by discriminant.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub encode_as: Option<TagEncoding>,
}

impl Quiddity for TaggedKind {
    fn encode(&self, e: &mut Encoder) {
        let mut f = e.fields();
        if let Some(v) = &self.content {
            f.field("content", v);
        }
        if let Some(v) = &self.encode_as {
            f.field("encode_as", v);
        }
        if let Some(v) = &self.style {
            f.field("style", v);
        }
        if let Some(v) = &self.tag {
            f.field("tag", v);
        }
        f.field("variants", &self.variants);
        f.finish();
    }
}

/// How a tagged union's variants are tagged.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
#[serde(rename_all = "lowercase")]
pub enum Style {
    /// `{"Variant": value}`, or `"Variant"` for a unit variant.
    External,
    /// `{"tag": "Variant", ...fields}`.
    Internal,
    /// `{"tag": "Variant", "content": value}`.
    Adjacent,
}

impl Quiddity for Style {
    fn encode(&self, e: &mut Encoder) {
        e.str(match self {
            Style::External => "external",
            Style::Internal => "internal",
            Style::Adjacent => "adjacent",
        });
    }
}

/// How a tagged union's variants are identified in the result.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
#[serde(rename_all = "lowercase")]
pub enum TagEncoding {
    /// By name.
    String,
    /// By discriminant.
    Int,
}

impl Quiddity for TagEncoding {
    fn encode(&self, e: &mut Encoder) {
        e.str(match self {
            TagEncoding::String => "string",
            TagEncoding::Int => "int",
        });
    }
}

/// A variant of a [`TaggedKind`].
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Variant {
    /// The variant's kind; `None` for a unit variant.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<Kind>,
    /// The variant's name in the result, if not its name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rename: Option<String>,
    /// The variant's discriminant, with `TagEncoding::Int`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub discriminant: Option<i128>,
}

impl Quiddity for Variant {
    fn encode(&self, e: &mut Encoder) {
        let mut f = e.fields();
        if let Some(v) = &self.discriminant {
            f.field("discriminant", v);
        }
        if let Some(v) = &self.kind {
            f.field("kind", v);
        }
        if let Some(v) = &self.rename {
            f.field("rename", v);
        }
        f.finish();
    }
}

impl Kind {
    /// A list of `item`.
    ///
    /// # Examples
    ///
    /// ```
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # use quiddity::identity::{Field, Identity, Kind, Omit, Unit};
    /// # use serde_json::json;
    /// let id = Identity::new(Kind::list(Kind::String))?;
    /// // Order matters.
    /// assert_ne!(id.digest(&json!(["a", "b"]))?, id.digest(&json!(["b", "a"]))?);
    /// # Ok(())
    /// # }
    /// ```
    pub fn list(item: impl Into<Kind>) -> Kind {
        Kind::List(Box::new(item.into()))
    }

    /// A set of `member`.
    ///
    /// # Examples
    ///
    /// ```
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # use quiddity::identity::{Field, Identity, Kind, Omit, Unit};
    /// # use serde_json::json;
    /// let id = Identity::new(Kind::set(Kind::String))?;
    /// // Order does not matter.
    /// assert_eq!(id.digest(&json!(["a", "b"]))?, id.digest(&json!(["b", "a"]))?);
    /// # Ok(())
    /// # }
    /// ```
    pub fn set(member: impl Into<Kind>) -> Kind {
        Kind::Set(Box::new(member.into()))
    }

    /// `kind` or null.
    ///
    /// # Examples
    ///
    /// ```
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # use quiddity::identity::{Field, Identity, Kind, Omit, Unit};
    /// # use serde_json::json;
    /// let id = Identity::new(Kind::optional(Kind::Integer))?;
    /// assert!(id.digest(&json!(null)).is_ok());
    /// assert!(id.digest(&json!(5)).is_ok());
    /// assert!(id.digest(&json!("five")).is_err());
    /// # Ok(())
    /// # }
    /// ```
    pub fn optional(kind: impl Into<Kind>) -> Kind {
        Kind::Union(alloc::vec![Kind::Null, kind.into()])
    }

    /// The identity this kind is placed in, for recursive types: a tree
    /// node's `children` field is `Kind::list(Kind::recursive())`.
    ///
    /// # Examples
    ///
    /// ```
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # use quiddity::identity::{Field, Identity, Kind, Omit, Unit};
    /// # use serde_json::json;
    /// // A tree: each node's children are nodes.
    /// let node = Identity::new(Kind::object([
    ///     ("value", Field::new(Kind::Integer)),
    ///     ("children", Field::new(Kind::list(Kind::recursive()))),
    /// ])?)?
    /// .named("Node");
    /// let tree = json!({"value": 1, "children": [{"value": 2, "children": []}]});
    /// assert!(node.digest(&tree).is_ok());
    /// # Ok(())
    /// # }
    /// ```
    pub fn recursive() -> Kind {
        Kind::Recursive
    }

    /// Bytes, read from text as base64.
    ///
    /// # Examples
    ///
    /// ```
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # use quiddity::identity::{Field, Identity, Kind, Omit, Unit};
    /// # use serde_json::json;
    /// let id = Identity::new(Kind::bytes())?;
    /// // An array of byte values and base64 text: the same bytes.
    /// assert_eq!(id.digest(&json!([104, 105]))?, id.digest(&json!("aGk="))?);
    /// # Ok(())
    /// # }
    /// ```
    pub fn bytes() -> Kind {
        Kind::Bytes(BytesOptions::default())
    }

    /// Bytes, read from text as hexadecimal digits.
    ///
    /// # Examples
    ///
    /// ```
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # use quiddity::identity::{Field, Identity, Kind, Omit, Unit};
    /// # use serde_json::json;
    /// let id = Identity::new(Kind::bytes_hex())?;
    /// assert_eq!(id.digest(&json!("6869"))?, id.digest(&json!([104, 105]))?);
    /// # Ok(())
    /// # }
    /// ```
    pub fn bytes_hex() -> Kind {
        Kind::Bytes(BytesOptions {
            text: Some(BytesText::Hex),
        })
    }

    /// An instant, read from RFC 3339 text or serde's `SystemTime` form.
    ///
    /// # Examples
    ///
    /// ```
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # use quiddity::identity::{Field, Identity, Kind, Omit, Unit};
    /// # use serde_json::json;
    /// let id = Identity::new(Kind::instant())?;
    /// let utc = id.digest(&json!("2024-01-01T00:00:00Z"))?;
    /// let paris = id.digest(&json!("2024-01-01T01:00:00+01:00"))?;
    /// assert_eq!(utc, paris);
    /// // A time without an offset is not an instant.
    /// assert!(id.digest(&json!("2024-01-01T00:00:00")).is_err());
    /// # Ok(())
    /// # }
    /// ```
    pub fn instant() -> Kind {
        Kind::Instant(TimeOptions::default())
    }

    /// An instant, also read from a number of `unit`s since the Unix epoch.
    ///
    /// # Examples
    ///
    /// ```
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # use quiddity::identity::{Field, Identity, Kind, Omit, Unit};
    /// # use serde_json::json;
    /// let id = Identity::new(Kind::instant_in(Unit::Ms))?;
    /// assert_eq!(
    ///     id.digest(&json!(1704067200000u64))?,
    ///     id.digest(&json!("2024-01-01T00:00:00Z"))?,
    /// );
    /// # Ok(())
    /// # }
    /// ```
    pub fn instant_in(unit: Unit) -> Kind {
        Kind::Instant(TimeOptions { unit: Some(unit) })
    }

    /// A duration, read from ISO 8601 text or serde's `Duration` form.
    ///
    /// # Examples
    ///
    /// ```
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # use quiddity::identity::{Field, Identity, Kind, Omit, Unit};
    /// # use serde_json::json;
    /// let id = Identity::new(Kind::duration())?;
    /// assert_eq!(id.digest(&json!("PT90S"))?, id.digest(&json!("PT1M30S"))?);
    /// // serde's form of a Rust `Duration`.
    /// assert_eq!(
    ///     id.digest(&json!("PT1.5S"))?,
    ///     id.digest(&json!({"secs": 1, "nanos": 500000000}))?,
    /// );
    /// # Ok(())
    /// # }
    /// ```
    pub fn duration() -> Kind {
        Kind::Duration(TimeOptions::default())
    }

    /// A duration, also read from a number of `unit`s.
    ///
    /// # Examples
    ///
    /// ```
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # use quiddity::identity::{Field, Identity, Kind, Omit, Unit};
    /// # use serde_json::json;
    /// let id = Identity::new(Kind::duration_in(Unit::S))?;
    /// assert_eq!(id.digest(&json!(90))?, id.digest(&json!("PT1M30S"))?);
    /// # Ok(())
    /// # }
    /// ```
    pub fn duration_in(unit: Unit) -> Kind {
        Kind::Duration(TimeOptions { unit: Some(unit) })
    }

    /// A map from `key` to `value`. In JSON every key is a string, so `key`
    /// must accept strings.
    ///
    /// # Examples
    ///
    /// ```
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # use quiddity::identity::{Field, Identity, Kind, Omit, Unit};
    /// # use serde_json::json;
    /// // Keys read as integers from JSON's string keys.
    /// let id = Identity::new(Kind::map(Kind::Integer, Kind::String)?)?;
    /// assert_eq!(
    ///     id.digest(&json!({"1": "a", "2": "b"}))?,
    ///     id.digest(&json!({"2": "b", "1": "a"}))?,
    /// );
    /// // A key kind must accept strings.
    /// assert!(Kind::map(Kind::list(Kind::String), Kind::Any).is_err());
    /// # Ok(())
    /// # }
    /// ```
    pub fn map(key: impl Into<Kind>, value: impl Into<Kind>) -> Result<Kind, InvalidIdentity> {
        compose::checked(Kind::Map(MapKind {
            key: Box::new(key.into()),
            value: Box::new(value.into()),
        }))
    }

    /// An object with the given fields, encoded as a map from their keys.
    ///
    /// # Examples
    ///
    /// ```
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # use quiddity::identity::{Field, Identity, Kind, Omit, Unit};
    /// # use serde_json::json;
    /// let id = Identity::new(Kind::object([("x", Kind::Integer), ("y", Kind::Integer)])?)?;
    /// // Members not listed are skipped, and member order does not matter.
    /// assert_eq!(
    ///     id.digest(&json!({"x": 1, "y": 2, "label": "a"}))?,
    ///     id.digest(&json!({"y": 2, "x": 1}))?,
    /// );
    /// // A missing member is null, which `Kind::Integer` does not accept.
    /// assert!(id.digest(&json!({"x": 1})).is_err());
    /// # Ok(())
    /// # }
    /// ```
    pub fn object<N, F>(fields: impl IntoIterator<Item = (N, F)>) -> Result<Kind, InvalidIdentity>
    where
        N: Into<String>,
        F: Into<Field>,
    {
        compose::object(fields, false)
    }

    /// An object with the given fields, encoded as the list of their values
    /// in the order given. Its fields cannot be omitted.
    ///
    /// # Examples
    ///
    /// ```
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # use quiddity::identity::{Field, Identity, Kind, Omit, Unit};
    /// # use serde_json::json;
    /// let point = Identity::new(Kind::object_as_list([("x", Kind::Integer), ("y", Kind::Integer)])?)?;
    /// let list = Identity::new(Kind::list(Kind::Integer))?;
    /// // Digested as the list [x, y].
    /// assert_eq!(point.digest(&json!({"y": 2, "x": 1}))?, list.digest(&json!([1, 2]))?);
    /// # Ok(())
    /// # }
    /// ```
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
    ///
    /// # Examples
    ///
    /// ```
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # use quiddity::identity::{Field, Identity, Kind, Omit, Unit};
    /// # use serde_json::json;
    /// let id = Identity::new(Kind::union([Kind::Integer, Kind::String])?)?;
    /// // Told apart by type: 5 and "5" stay different.
    /// assert_ne!(id.digest(&json!(5))?, id.digest(&json!("5"))?);
    /// // Two kinds that both read numbers cannot be told apart.
    /// assert!(Kind::union([Kind::Integer, Kind::Number]).is_err());
    ///
    /// // Objects are told apart by their keys.
    /// let figure = Identity::new(Kind::union([
    ///     Kind::object([("x", Kind::Integer), ("y", Kind::Integer)])?,
    ///     Kind::object([("w", Kind::Integer), ("h", Kind::Integer)])?,
    /// ])?)?;
    /// assert!(figure.digest(&json!({"w": 1, "h": 2})).is_ok());
    /// assert!(figure.digest(&json!({"x": 1, "h": 2})).is_err());
    /// # Ok(())
    /// # }
    /// ```
    pub fn union<K: Into<Kind>>(
        alternatives: impl IntoIterator<Item = K>,
    ) -> Result<Kind, InvalidIdentity> {
        compose::checked(Kind::Union(
            alternatives.into_iter().map(Into::into).collect(),
        ))
    }

    /// A fixed-length array, each item of the kind in its position.
    ///
    /// # Examples
    ///
    /// ```
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # use quiddity::identity::{Field, Identity, Kind, Omit, Unit};
    /// # use serde_json::json;
    /// let id = Identity::new(Kind::tuple([Kind::String, Kind::Integer])?)?;
    /// assert!(id.digest(&json!(["a", 1])).is_ok());
    /// assert!(id.digest(&json!(["a", 1, 2])).is_err());
    /// # Ok(())
    /// # }
    /// ```
    pub fn tuple<K: Into<Kind>>(
        items: impl IntoIterator<Item = K>,
    ) -> Result<Kind, InvalidIdentity> {
        compose::checked(Kind::Tuple(items.into_iter().map(Into::into).collect()))
    }

    /// Variants written as `{"Name": value}`, or `"Name"` for a unit
    /// variant (`None`), as serde writes enums by default.
    ///
    /// # Examples
    ///
    /// ```
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # use quiddity::identity::{Field, Identity, Kind, Omit, Unit};
    /// # use serde_json::json;
    /// let id = Identity::new(Kind::externally_tagged([
    ///     ("Empty", None),
    ///     ("Circle", Some(Kind::Number)),
    /// ])?)?;
    /// assert!(id.digest(&json!("Empty")).is_ok());
    /// assert!(id.digest(&json!({"Circle": 1.5})).is_ok());
    /// # Ok(())
    /// # }
    /// ```
    pub fn externally_tagged<N: Into<String>>(
        variants: impl IntoIterator<Item = (N, Option<Kind>)>,
    ) -> Result<Kind, InvalidIdentity> {
        compose::tagged(variants, None, None, None)
    }

    /// Variants written as objects whose member `tag` names the variant,
    /// as serde's `#[serde(tag = "…")]` does. Each variant is a unit
    /// variant (`None`) or an object kind.
    ///
    /// # Examples
    ///
    /// ```
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # use quiddity::identity::{Field, Identity, Kind, Omit, Unit};
    /// # use serde_json::json;
    /// let rect = Kind::object([("w", Kind::Integer), ("h", Kind::Integer)])?;
    /// let id = Identity::new(Kind::internally_tagged("type", [
    ///     ("Empty", None),
    ///     ("Rect", Some(rect)),
    /// ])?)?;
    /// assert!(id.digest(&json!({"type": "Rect", "w": 1, "h": 2})).is_ok());
    /// # Ok(())
    /// # }
    /// ```
    pub fn internally_tagged<N: Into<String>>(
        tag: impl Into<String>,
        variants: impl IntoIterator<Item = (N, Option<Kind>)>,
    ) -> Result<Kind, InvalidIdentity> {
        compose::tagged(variants, Some(Style::Internal), Some(tag.into()), None)
    }

    /// Variants written as `{tag: "Name", content: value}`, as serde's
    /// `#[serde(tag = "…", content = "…")]` does.
    ///
    /// # Examples
    ///
    /// ```
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # use quiddity::identity::{Field, Identity, Kind, Omit, Unit};
    /// # use serde_json::json;
    /// let id = Identity::new(Kind::adjacently_tagged("t", "c", [
    ///     ("Empty", None),
    ///     ("Circle", Some(Kind::Number)),
    /// ])?)?;
    /// assert!(id.digest(&json!({"t": "Circle", "c": 1.5})).is_ok());
    /// # Ok(())
    /// # }
    /// ```
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
    ///
    /// # Examples
    ///
    /// ```
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # use quiddity::identity::{Field, Identity, Kind, Omit, Unit};
    /// # use serde_json::json;
    /// let level = Kind::externally_tagged([("Low", None), ("High", None)])?
    ///     .by_discriminant([("Low", 0), ("High", 1)])?;
    /// let id = Identity::new(level)?;
    /// // "High" is digested as the integer 1.
    /// assert_eq!(id.digest(&json!("High"))?, Identity::new(Kind::Integer)?.digest(&json!(1))?);
    /// # Ok(())
    /// # }
    /// ```
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
            quantize: None,
            decimals: None,
        }
    }

    /// A field of `kind` or null, omitted when null: an `Option` that
    /// serde leaves out when it is `None`.
    ///
    /// # Examples
    ///
    /// ```
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # use quiddity::identity::{Field, Identity, Kind, Omit, Unit};
    /// # use serde_json::json;
    /// let id = Identity::new(Kind::object([("note", Field::optional(Kind::String))])?)?;
    /// // A missing note and a null one are both left out.
    /// assert_eq!(id.digest(&json!({}))?, id.digest(&json!({"note": null}))?);
    /// # Ok(())
    /// # }
    /// ```
    pub fn optional(kind: impl Into<Kind>) -> Field {
        Field::new(Kind::optional(kind)).omit(Omit::None)
    }

    /// A field omitted when it is empty or missing: a collection that
    /// serde leaves out when it is empty.
    ///
    /// # Examples
    ///
    /// ```
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # use quiddity::identity::{Field, Identity, Kind, Omit, Unit};
    /// # use serde_json::json;
    /// let id = Identity::new(Kind::object([("tags", Field::omit_empty(Kind::set(Kind::String)))])?)?;
    /// assert_eq!(id.digest(&json!({}))?, id.digest(&json!({"tags": []}))?);
    /// # Ok(())
    /// # }
    /// ```
    pub fn omit_empty(kind: impl Into<Kind>) -> Field {
        Field::new(Kind::optional(kind))
            .omit(Omit::None)
            .omit(Omit::Empty)
    }

    /// The field, with its key in the result changed to `key`.
    ///
    /// # Examples
    ///
    /// ```
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # use quiddity::identity::{Field, Identity, Kind, Omit, Unit};
    /// # use serde_json::json;
    /// // Read from the member `kind`, digested under the key `type`.
    /// let id = Identity::new(Kind::object([("kind", Field::new(Kind::String).rename("type"))])?)?;
    /// let map = Identity::new(Kind::map(Kind::String, Kind::String)?)?;
    /// assert_eq!(id.digest(&json!({"kind": "q"}))?, map.digest(&json!({"type": "q"}))?);
    /// # Ok(())
    /// # }
    /// ```
    pub fn rename(mut self, key: impl Into<String>) -> Field {
        self.rename = Some(key.into());
        self
    }

    /// The field, left out when `condition` holds.
    ///
    /// # Examples
    ///
    /// ```
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # use quiddity::identity::{Field, Identity, Kind, Omit, Unit};
    /// # use serde_json::json;
    /// let field = Field::new(Kind::optional(Kind::String)).omit(Omit::None);
    /// assert_eq!(field, Field::optional(Kind::String));
    /// # Ok(())
    /// # }
    /// ```
    pub fn omit(mut self, condition: Omit) -> Field {
        self.omit.insert(condition);
        self
    }

    /// The field, with its value (an integer, instant or duration) rounded
    /// down to a multiple of `step`, in nanoseconds for instants and
    /// durations (spec 5.6).
    ///
    /// # Examples
    ///
    /// ```
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # use quiddity::identity::{Field, Identity, Kind};
    /// # use serde_json::json;
    /// let id = Identity::new(Kind::object([("count", Field::new(Kind::Integer).quantize(100))])?)?;
    /// // 100 to 199 are one value; 99 is another.
    /// assert_eq!(id.digest(&json!({"count": 100}))?, id.digest(&json!({"count": 149}))?);
    /// assert_ne!(id.digest(&json!({"count": 99}))?, id.digest(&json!({"count": 100}))?);
    ///
    /// // Instants to the second.
    /// let at = Identity::new(Kind::object([("at", Field::new(Kind::instant()).quantize(1_000_000_000))])?)?;
    /// assert_eq!(
    ///     at.digest(&json!({"at": "2024-01-01T00:00:00Z"}))?,
    ///     at.digest(&json!({"at": "2024-01-01T00:00:00.75Z"}))?,
    /// );
    /// # Ok(())
    /// # }
    /// ```
    pub fn quantize(mut self, step: u64) -> Field {
        self.quantize = Some(step);
        self
    }

    /// The field, with its value (a number) rounded down to a multiple of
    /// 10^−`decimals` (spec 5.6). A float is taken as the shortest decimal
    /// that rounds to it, as JSON writes it.
    ///
    /// # Examples
    ///
    /// ```
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # use quiddity::identity::{Field, Identity, Kind};
    /// # use serde_json::json;
    /// let id = Identity::new(Kind::object([("price", Field::new(Kind::Number).decimals(2))])?)?;
    /// assert_eq!(id.digest(&json!({"price": 12.98}))?, id.digest(&json!({"price": 12.989}))?);
    /// assert_eq!(id.digest(&json!({"price": 2.67}))?, id.digest(&json!({"price": 2.675}))?);
    /// # Ok(())
    /// # }
    /// ```
    pub fn decimals(mut self, decimals: i32) -> Field {
        self.decimals = Some(decimals);
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
    ///
    /// # Examples
    ///
    /// ```
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # use quiddity::identity::{Field, Identity, Kind, Omit, Unit};
    /// # use serde_json::json;
    /// let id = Identity::new(Kind::object([("prompt", Kind::String)])?)?;
    /// assert!(id.digest(&json!({"prompt": "Summarize the report"})).is_ok());
    /// // An identity is checked when it is made.
    /// assert!(Identity::new(Kind::recursive()).is_err());
    /// # Ok(())
    /// # }
    /// ```
    pub fn new(kind: impl Into<Kind>) -> Result<Identity, InvalidIdentity> {
        compose::identity(kind.into(), None)
    }

    /// The identity, named `name`: its root becomes its own definition
    /// under that name, which is how the identity appears in its document
    /// and in the documents of identities it is nested in. An identity that
    /// is not named is written in place where it is nested.
    ///
    /// # Examples
    ///
    /// ```
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # use quiddity::identity::{Field, Identity, Kind, Omit, Unit};
    /// # use serde_json::json;
    /// let row = Identity::new(Kind::object([("label", Kind::String)])?)?.named("Row");
    /// let table = Identity::new(Kind::list(row))?;
    /// // The nested identity is a definition, under its name.
    /// let document = serde_json::to_value(&table)?;
    /// assert_eq!(document["root"], json!({"list": {"ref": "Row"}}));
    /// # Ok(())
    /// # }
    /// ```
    pub fn named(self, name: impl Into<String>) -> Identity {
        compose::named(self, name.into())
    }

    /// The name of the identity's own definition, if its root is one: the
    /// name given with [`Identity::named`], a type's name for an exported
    /// identity, or `_self` for an unnamed recursive identity.
    ///
    /// # Examples
    ///
    /// ```
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # use quiddity::identity::{Field, Identity, Kind, Omit, Unit};
    /// # use serde_json::json;
    /// assert_eq!(Identity::new(Kind::String)?.named("Label").name(), Some("Label"));
    /// assert_eq!(Identity::new(Kind::String)?.name(), None);
    /// # Ok(())
    /// # }
    /// ```
    pub fn name(&self) -> Option<&str> {
        match &self.root {
            Kind::Ref(name) => Some(name.as_str()),
            _ => None,
        }
    }

    /// The identity's root kind.
    ///
    /// # Examples
    ///
    /// ```
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # use quiddity::identity::{Field, Identity, Kind, Omit, Unit};
    /// # use serde_json::json;
    /// let id = Identity::new(Kind::list(Kind::String))?;
    /// assert_eq!(id.root(), &Kind::list(Kind::String));
    /// # Ok(())
    /// # }
    /// ```
    pub fn root(&self) -> &Kind {
        &self.root
    }

    /// The identity's definitions, by name.
    ///
    /// # Examples
    ///
    /// ```
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # use quiddity::identity::{Field, Identity, Kind, Omit, Unit};
    /// # use serde_json::json;
    /// let row = Identity::new(Kind::object([("label", Kind::String)])?)?.named("Row");
    /// let table = Identity::new(Kind::list(row))?;
    /// assert!(table.definitions().contains_key("Row"));
    /// # Ok(())
    /// # }
    /// ```
    pub fn definitions(&self) -> &BTreeMap<String, Kind> {
        &self.definitions
    }

    /// The definition `name`, as an identity of its own.
    ///
    /// # Examples
    ///
    /// ```
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # use quiddity::identity::{Field, Identity, Kind, Omit, Unit};
    /// # use serde_json::json;
    /// let row = Identity::new(Kind::object([("label", Kind::String)])?)?.named("Row");
    /// let table = Identity::new(Kind::list(row.clone()))?;
    /// assert_eq!(table.definition("Row"), Some(row));
    /// # Ok(())
    /// # }
    /// ```
    pub fn definition(&self, name: &str) -> Option<Identity> {
        compose::definition(self, name)
    }

    /// The identity's domain, if any.
    ///
    /// # Examples
    ///
    /// ```
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # use quiddity::identity::{Field, Identity, Kind, Omit, Unit};
    /// # use serde_json::json;
    /// let mut id = Identity::new(Kind::String)?;
    /// id.set_domain("example.label")?;
    /// assert_eq!(id.domain(), Some("example.label"));
    /// # Ok(())
    /// # }
    /// ```
    pub fn domain(&self) -> Option<&str> {
        self.domain.as_deref()
    }

    /// Places digests under this identity in `domain` (spec 4.2), which
    /// must not be empty.
    ///
    /// # Examples
    ///
    /// ```
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # use quiddity::identity::{Field, Identity, Kind, Omit, Unit};
    /// # use serde_json::json;
    /// let fields = Kind::object([("to", Kind::String), ("amount", Kind::Integer)])?;
    /// let mut payment = Identity::new(fields)?;
    /// let mut refund = payment.clone();
    /// payment.set_domain("example.payment")?;
    /// refund.set_domain("example.refund")?;
    /// // The same fields, in different domains: different digests.
    /// let value = json!({"to": "bob", "amount": 100});
    /// assert_ne!(payment.digest(&value)?, refund.digest(&value)?);
    /// # Ok(())
    /// # }
    /// ```
    pub fn set_domain(&mut self, domain: impl Into<String>) -> Result<(), InvalidIdentity> {
        let domain = domain.into();
        if domain.is_empty() {
            return Err(load::invalid("$.domain", "a domain is non-empty"));
        }
        self.domain = Some(domain);
        Ok(())
    }

    /// Removes the identity's domain.
    ///
    /// # Examples
    ///
    /// ```
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # use quiddity::identity::{Field, Identity, Kind, Omit, Unit};
    /// # use serde_json::json;
    /// let mut id = Identity::new(Kind::String)?;
    /// id.set_domain("example.label")?;
    /// id.clear_domain();
    /// assert_eq!(id.domain(), None);
    /// # Ok(())
    /// # }
    /// ```
    pub fn clear_domain(&mut self) {
        self.domain = None;
    }

    /// Replaces the root kind.
    ///
    /// # Examples
    ///
    /// ```
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # use quiddity::identity::{Field, Identity, Kind, Omit, Unit};
    /// # use serde_json::json;
    /// let mut id = Identity::new(Kind::String)?;
    /// id.set_root(Kind::Integer)?;
    /// assert!(id.digest(&json!(5)).is_ok());
    /// # Ok(())
    /// # }
    /// ```
    pub fn set_root(&mut self, kind: impl Into<Kind>) -> Result<(), InvalidIdentity> {
        let mut next = compose::identity(kind.into(), self.domain.clone())?;
        if let Some(name) = self.name() {
            next = compose::named(next, String::from(name));
        }
        *self = next;
        Ok(())
    }

    /// Adds or replaces the field `name` of the root object.
    ///
    /// # Examples
    ///
    /// ```
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # use quiddity::identity::{Field, Identity, Kind, Omit, Unit};
    /// # use serde_json::json;
    /// let mut id = Identity::new(Kind::object([("prompt", Kind::String)])?)?;
    /// id.set_field("priority", Kind::Integer)?;
    /// assert!(id.digest(&json!({"prompt": "p", "priority": 1})).is_ok());
    /// // A change that would make the identity invalid fails, and changes nothing.
    /// assert!(id.set_field("other", Field::new(Kind::String).rename("prompt")).is_err());
    /// # Ok(())
    /// # }
    /// ```
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
    ///
    /// # Examples
    ///
    /// ```
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # use quiddity::identity::{Field, Identity, Kind, Omit, Unit};
    /// # use serde_json::json;
    /// let mut id = Identity::new(Kind::object([("prompt", Kind::String), ("priority", Kind::Integer)])?)?;
    /// id.remove_field("priority")?;
    /// assert!(id.digest(&json!({"prompt": "p"})).is_ok());
    /// # Ok(())
    /// # }
    /// ```
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
    ///
    /// # Examples
    ///
    /// ```
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # use quiddity::identity::{Field, Identity, Kind, Omit, Unit};
    /// # use serde_json::json;
    /// let row = Identity::new(Kind::object([("label", Kind::String)])?)?.named("Row");
    /// let mut table = Identity::new(Kind::list(row))?;
    /// table.set_field_in("Row", "value", Kind::Number)?;
    /// assert!(table.digest(&json!([{"label": "x", "value": 1.5}])).is_ok());
    /// # Ok(())
    /// # }
    /// ```
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
    ///
    /// # Examples
    ///
    /// ```
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # use quiddity::identity::{Field, Identity, Kind, Omit, Unit};
    /// # use serde_json::json;
    /// let row = Identity::new(Kind::object([("label", Kind::String)])?)?.named("Row");
    /// let mut q = Identity::new(Kind::object([("brief", Field::new(Kind::String)), ("rows", Field::new(Kind::list(row)))])?)?
    ///     .named("Question");
    /// // Removing the last use of `Row` removes its definition.
    /// q.remove_field_in("Question", "rows")?;
    /// assert!(!q.definitions().contains_key("Row"));
    /// # Ok(())
    /// # }
    /// ```
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

    /// Loads an identity document (spec 5.3). Any serde format reads one
    /// through `Identity`'s `Deserialize` impl, which does the same.
    ///
    /// # Examples
    ///
    /// ```
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # use quiddity::identity::{Field, Identity, Kind, Omit, Unit};
    /// # use serde_json::json;
    /// let id = Identity::from_json(&json!({
    ///     "quiddity": 1,
    ///     "root": {"object": {"fields": {"prompt": {"kind": "string"}}}}
    /// }))?;
    /// assert_eq!(id, Identity::new(Kind::object([("prompt", Kind::String)])?)?);
    /// // A document that is not valid is an error.
    /// assert!(Identity::from_json(&json!({"quiddity": 1, "root": "text"})).is_err());
    /// # Ok(())
    /// # }
    /// ```
    #[cfg(feature = "serde_json")]
    pub fn from_json(document: &serde_json::Value) -> Result<Identity, InvalidIdentity> {
        let doc = <load::Document as serde::Deserialize>::deserialize(document)
            .map_err(|e| load::invalid("$", alloc::string::ToString::to_string(&e)))?;
        Identity::try_from(doc)
    }

    /// The identity document, in canonical form. Any serde format writes
    /// one through `Identity`'s `Serialize` impl, which does the same.
    ///
    /// # Examples
    ///
    /// ```
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # use quiddity::identity::{Field, Identity, Kind, Omit, Unit};
    /// # use serde_json::json;
    /// let id = Identity::new(Kind::optional(Kind::String))?;
    /// assert_eq!(id.to_json(), json!({"quiddity": 1, "root": {"union": ["null", "string"]}}));
    /// assert_eq!(Identity::from_json(&id.to_json())?, id);
    /// # Ok(())
    /// # }
    /// ```
    #[cfg(feature = "serde_json")]
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::to_value(self).expect("an identity document is valid JSON")
    }

    /// The digest of `value` under this identity (spec 5.1).
    ///
    /// `value` is read in the form serde_json gives it, so it has the digest
    /// of its `serde_json::Value`. Where a `serde_json::Value` cannot hold a
    /// value, it is kept: integers beyond 64 bits, and non-finite floats.
    ///
    /// # Errors
    ///
    /// Returns an error if `value` does not have the shape this identity
    /// describes, or if its `Serialize` impl fails.
    ///
    /// # Examples
    ///
    /// ```
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # use quiddity::identity::{Field, Identity, Kind, Omit, Unit};
    /// # use serde_json::json;
    /// let id = Identity::new(Kind::object([("prompt", Kind::String)])?)?;
    /// let digest = id.digest(&json!({"prompt": "Summarize the report"}))?;
    /// assert!(digest.to_string().starts_with("sha256:"));
    /// // A value of the wrong shape is an error that says where.
    /// let err = id.digest(&json!({"prompt": 5})).unwrap_err();
    /// assert_eq!(err.path, "$.prompt");
    /// # Ok(())
    /// # }
    /// ```
    pub fn digest<T: serde::Serialize + ?Sized>(&self, value: &T) -> Result<Digest, ApplyError> {
        let v = apply::apply(self, &Self::source(value)?)?;
        let mut e = crate::Encoder::new();
        v.encode(&mut e);
        let structural = e.finish();
        Ok(match &self.domain {
            Some(label) => structural.in_domain(label),
            None => structural,
        })
    }

    /// The source tree of `value`; a failing `Serialize` impl is an error
    /// at the value itself.
    fn source<T: serde::Serialize + ?Sized>(value: &T) -> Result<source::Src, ApplyError> {
        source::to_source(value).map_err(|e| ApplyError {
            path: "$".into(),
            message: alloc::format!("the value could not be serialized: {e}"),
        })
    }

    /// The encoding of `value` under this identity.
    #[cfg(test)]
    pub(crate) fn encoding<T: serde::Serialize + ?Sized>(
        &self,
        value: &T,
    ) -> Result<Vec<u8>, ApplyError> {
        let v = apply::apply(self, &Self::source(value)?)?;
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

impl core::error::Error for ApplyError {}
