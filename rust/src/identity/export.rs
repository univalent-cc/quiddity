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

/// A type or attribute that an identity cannot express.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExportError {
    /// Where: the exported type's name, then the names of the fields and
    /// variants leading to the problem, separated by `.`; empty if the
    /// problem is the identity as a whole.
    pub path: String,
    /// What cannot be exported, and why.
    pub message: String,
}

impl fmt::Display for ExportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.path.is_empty() {
            write!(f, "cannot export an identity: {}", self.message)
        } else {
            write!(
                f,
                "cannot export an identity: at {}: {}",
                self.path, self.message
            )
        }
    }
}

impl core::error::Error for ExportError {}

/// Collects the definitions of the types an export reaches.
pub struct Exporter {
    definitions: BTreeMap<String, Kind>,
    /// The definition name of each (type, identity) reached so far.
    names: BTreeMap<(&'static str, &'static str), String>,
    /// The root type, and the fields and variants being described.
    path: Vec<String>,
}

/// The identity of `T` under `I` (spec 6.8).
pub fn construct<T: Quiddity<I> + ?Sized, I: IdentityMarker>() -> Result<Identity, ExportError> {
    let mut cx = Exporter {
        definitions: BTreeMap::new(),
        names: BTreeMap::new(),
        path: Vec::new(),
    };
    let root = cx.describe::<T, I>()?;
    let domain = <T as Quiddity<I>>::DOMAIN.map(ToOwned::to_owned);
    Identity::from_parts(root, cx.definitions, domain).map_err(|e| ExportError {
        path: String::new(),
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
            return Ok(Kind::Ref(super::RefName::new(name.clone())));
        }
        let suffix = if key.1 == type_name::<Primary>() {
            String::new()
        } else {
            format!("/{}", short_name(key.1))
        };
        let mut name = format!("{}{suffix}", short_name(key.0));
        if self.names.values().any(|n| *n == name) || self.definitions.contains_key(&name) {
            name = format!("{}{suffix}", key.0);
        }
        self.names.insert(key, name.clone());
        let root = self.path.is_empty();
        if root {
            self.path.push(short_name(key.0).to_owned());
        }
        let kind = build(self);
        if root {
            self.path.pop();
        }
        self.definitions.insert(name.clone(), kind?);
        Ok(Kind::Ref(super::RefName::new(name)))
    }

    /// The kind standing for the identity `id`, whose definitions join
    /// the export's: a reference to its own definition if it is named,
    /// else its root. Names taken by other types are not reused.
    pub fn nest(&mut self, id: Identity) -> Kind {
        // Reserve the names of types still being described, whose
        // definitions are not yet added.
        let pending: Vec<String> = self
            .names
            .values()
            .filter(|n| !self.definitions.contains_key(*n))
            .cloned()
            .collect();
        for n in &pending {
            self.definitions.insert(n.clone(), Kind::Recursive);
        }
        let kind = super::compose::merge(id, &mut self.definitions);
        for n in &pending {
            self.definitions.remove(n);
        }
        kind
    }

    /// `f`'s result, with `step` (a field's or variant's name) added to
    /// the path of any error it reports.
    pub fn within(
        &mut self,
        step: &str,
        f: impl FnOnce(&mut Self) -> Result<Kind, ExportError>,
    ) -> Result<Kind, ExportError> {
        self.path.push(step.to_owned());
        let r = f(self);
        self.path.pop();
        r
    }

    fn error(&self, message: String) -> ExportError {
        ExportError {
            path: self.path.join("."),
            message,
        }
    }

    /// The error for a type that does not describe itself.
    pub fn unsupported_type(&self, type_name: &str) -> ExportError {
        self.error(format!("`{type_name}` cannot be described by an identity"))
    }

    /// The error for an attribute that an identity cannot express.
    pub fn unsupported_attribute(&self, type_name: &str, attribute: &str) -> ExportError {
        self.error(format!(
            "`{type_name}` uses `{attribute}`, which an identity cannot express"
        ))
    }
}

// Builders used by the derive's expansion.

/// A field of an object kind. A field that omits empty values may be
/// missing from the JSON form (as with serde's `skip_serializing_if`), so it
/// also accepts null and omits it (spec 6.8).
pub fn field(kind: Kind, omit_none: bool, omit_empty: bool) -> Field {
    field_quantized(kind, omit_none, omit_empty, None, None)
}

/// A field of an object kind, with its quantization (spec 6.8).
pub fn field_quantized(
    kind: Kind,
    omit_none: bool,
    omit_empty: bool,
    quantize: Option<u64>,
    decimals: Option<i32>,
) -> Field {
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
        quantize,
        decimals,
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

/// An object kind from its fields and the kinds of its flattened fields
/// (D21), each named for errors: a flattened struct's fields join the
/// object's. Other flattened kinds (a map, an enum, an `Option`) cannot be
/// expressed in an identity.
pub fn object_flattened(
    cx: &Exporter,
    fields: Vec<(&'static str, Field)>,
    flattened: Vec<(&'static str, Kind)>,
) -> Result<Kind, ExportError> {
    let mut all: BTreeMap<String, Field> =
        fields.into_iter().map(|(n, f)| (n.to_owned(), f)).collect();
    for (name, kind) in flattened {
        let inner = match &kind {
            Kind::Ref(r) => cx.definitions.get(r.as_str()),
            _ => None,
        };
        let Some(Kind::Object(ObjectKind {
            fields,
            order: None,
        })) = inner
        else {
            return Err(ExportError {
                path: cx.path.join("."),
                message: format!(
                    "the flattened field `{name}` is not a struct with named fields; only those \
                     can be flattened in an identity"
                ),
            });
        };
        for (n, f) in fields {
            if all.insert(n.clone(), f.clone()).is_some() {
                return Err(ExportError {
                    path: cx.path.join("."),
                    message: format!(
                        "the flattened field `{name}` has a member `{n}`, which another field \
                         also has"
                    ),
                });
            }
        }
    }
    Ok(Kind::Object(ObjectKind {
        fields: all,
        order: None,
    }))
}

/// A map kind, not checked until the whole identity is (export refers to
/// definitions it has not finished collecting).
pub fn map(key: Kind, value: Kind) -> Kind {
    Kind::Map(super::MapKind {
        key: alloc::boxed::Box::new(key),
        value: alloc::boxed::Box::new(value),
    })
}

/// The kind of a field with `encode_as = set`: a set of the items of the
/// sequence its type describes.
pub fn as_set(cx: &Exporter, kind: Kind, type_name: &str) -> Result<Kind, ExportError> {
    match kind {
        Kind::List(item) | Kind::Set(item) => Ok(Kind::Set(item)),
        _ => Err(cx.error(format!(
            "`{type_name}` has `encode_as = set` but is not a sequence"
        ))),
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
