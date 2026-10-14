//! Exporting identities from Rust types (spec 6.8). Used by the
//! `Quiddity` implementations, including the derive's; not a stable API.

use alloc::borrow::ToOwned;
use alloc::collections::{BTreeMap, BTreeSet};
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use core::any::type_name;
use core::fmt;

use super::{Field, Identity, Kind, ObjectKind, Omit, TaggedKind, Variant};
use crate::{IdentityMarker, Primary, Quiddity};

/// A type or attribute that an identity document cannot express.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExportError {
    /// What cannot be exported, and why.
    pub message: String,
}

impl fmt::Display for ExportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "cannot export an identity: {}", self.message)
    }
}

#[cfg(feature = "std")]
impl std::error::Error for ExportError {}

/// Collects the definitions of the types an export reaches.
pub struct Exporter {
    definitions: BTreeMap<String, Kind>,
    /// The definition name of each (type, identity) reached so far.
    names: BTreeMap<(&'static str, &'static str), String>,
}

/// The identity of `T` under `I` (spec 6.8).
pub fn construct<T: Quiddity<I> + ?Sized, I: IdentityMarker>() -> Result<Identity, ExportError> {
    let mut cx = Exporter {
        definitions: BTreeMap::new(),
        names: BTreeMap::new(),
    };
    let root = cx.describe::<T, I>()?;
    let domain = <T as Quiddity<I>>::DOMAIN.map(ToOwned::to_owned);
    Identity::from_parts(root, cx.definitions, domain).map_err(|e| ExportError {
        message: format!("the identity of `{}` is not valid: {e}", type_name::<T>()),
    })
}

/// The last segment of a type's path, without generic arguments.
fn short_name(full: &'static str) -> &'static str {
    let base = full.split('<').next().unwrap_or(full);
    base.rsplit("::").next().unwrap_or(base)
}

impl Exporter {
    /// The kind of `T` under identity `J`.
    pub fn describe<T: Quiddity<J> + ?Sized, J: IdentityMarker>(
        &mut self,
    ) -> Result<Kind, ExportError> {
        T::__describe(self)
    }

    /// A reference to the definition of the derived type `T` under `J`,
    /// built by `build` the first time; a recursive type refers to itself.
    /// The name is the type's name, followed by `/` and the identity's name
    /// if that is not `Primary`; types with the same name are told apart by
    /// their full paths.
    pub fn define<T: ?Sized, J: IdentityMarker>(
        &mut self,
        build: impl FnOnce(&mut Self) -> Result<Kind, ExportError>,
    ) -> Result<Kind, ExportError> {
        let key = (type_name::<T>(), type_name::<J>());
        if let Some(name) = self.names.get(&key) {
            return Ok(Kind::Ref(name.clone()));
        }
        let suffix = if key.1 == type_name::<Primary>() {
            String::new()
        } else {
            format!("/{}", short_name(key.1))
        };
        let mut name = format!("{}{suffix}", short_name(key.0));
        if self.names.values().any(|n| *n == name) {
            name = format!("{}{suffix}", key.0);
        }
        self.names.insert(key, name.clone());
        let kind = build(self)?;
        self.definitions.insert(name.clone(), kind);
        Ok(Kind::Ref(name))
    }

    /// The error for a type that does not describe itself.
    pub fn unsupported_type(&self, type_name: &str) -> ExportError {
        ExportError {
            message: format!("`{type_name}` has no identity document form"),
        }
    }

    /// The error for an attribute that has no identity document form.
    pub fn unsupported_attribute(&self, type_name: &str, attribute: &str) -> ExportError {
        ExportError {
            message: format!(
                "`{type_name}` uses `{attribute}`, which has no identity document form"
            ),
        }
    }
}

// Builders used by the derive's expansion.

/// A field of an object kind. A field that omits empty values may be
/// missing from the JSON form (as with serde's `skip_serializing_if`), so it
/// also accepts null and omits it (spec 6.8).
pub fn field(kind: Kind, omit_none: bool, omit_empty: bool) -> Field {
    let mut omit = BTreeSet::new();
    let kind = if omit_empty {
        omit.insert(Omit::Empty);
        omit.insert(Omit::None);
        Kind::optional(kind)
    } else {
        kind
    };
    if omit_none {
        omit.insert(Omit::None);
    }
    Field {
        kind,
        rename: None,
        omit,
    }
}

/// An object kind from its fields, and its order if it is encoded as a
/// list.
pub fn object(fields: Vec<(&'static str, Field)>, list: bool) -> Kind {
    let order = list.then(|| fields.iter().map(|(n, _)| (*n).to_owned()).collect());
    Kind::Object(ObjectKind {
        fields: fields.into_iter().map(|(n, f)| (n.to_owned(), f)).collect(),
        order,
    })
}

/// The kind of a field with `encode_as = set`: a set of the items of the
/// sequence its type describes.
pub fn as_set(kind: Kind, type_name: &str) -> Result<Kind, ExportError> {
    match kind {
        Kind::List(item) | Kind::Set(item) => Ok(Kind::Set(item)),
        _ => Err(ExportError {
            message: format!("`{type_name}` has `encode_as = set` but is not a sequence"),
        }),
    }
}

/// A variant of a tagged union.
pub fn variant(
    name: &'static str,
    kind: Option<Kind>,
    discriminant: Option<i128>,
) -> (String, Variant) {
    (
        name.to_owned(),
        Variant {
            kind,
            rename: None,
            discriminant,
        },
    )
}

/// A tagged union kind.
pub fn tagged(
    variants: Vec<(String, Variant)>,
    style: Option<super::Style>,
    tag: Option<&'static str>,
    content: Option<&'static str>,
    int: bool,
) -> Kind {
    Kind::Tagged(TaggedKind {
        variants: variants.into_iter().collect(),
        style,
        tag: tag.map(ToOwned::to_owned),
        content: content.map(ToOwned::to_owned),
        encode_as: int.then_some(super::TagEncoding::Int),
    })
}
