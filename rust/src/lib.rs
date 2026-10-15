// Overrides for `docs.rs` links in the README. The first definition takes precedence.

// Fix links that are broken when documented without a feature:
#![cfg_attr(
    not(feature = "serde"),
    doc = "[`quiddity::serde::digest`]: #cargo-features"
)]
#![cfg_attr(
    feature = "serde",
    doc = "[`quiddity::serde::digest`]: crate::serde::digest"
)]
#![cfg_attr(not(feature = "identity"), doc = "[`identity::Kind`]: #cargo-features")]
#![cfg_attr(
    not(feature = "identity"),
    doc = "[`identity::Field`]: #cargo-features"
)]
#![cfg_attr(not(feature = "identity"), doc = "[`Identity`]: #cargo-features")]
#![cfg_attr(
    feature = "identity",
    doc = "[`identity::Kind`]: crate::identity::Kind"
)]
#![cfg_attr(
    feature = "identity",
    doc = "[`identity::Field`]: crate::identity::Field"
)]
#![cfg_attr(feature = "identity", doc = "[`Identity`]: crate::identity::Identity")]
// Normal links to crate items:
//! [`Quiddity`]: trait@Quiddity
//! [`Digest`]: Digest
//! [`Digest128`]: Digest128
//! [`Digest64`]: Digest64
//! [`Digest::mini()`]: Digest::mini
//! [`Digest::micro()`]: Digest::micro
//! [`Digest::in_domain`]: Digest::in_domain
//! [`IdentityMarker`]: trait@IdentityMarker
//! [`Primary`]: Primary
//! [`Encoder`]: Encoder
//! [`Encoder::bytes`]: Encoder::bytes

// The README is the crate's documentation. Its examples need `std` and
// `derive`, so they are tested from `ReadmeDoctests` below, with those features.
#![cfg_attr(not(doctest), doc = include_str!("../README.md"))]
// `docs.rs` builds with a nightly compiler, so it can label feature-gated items.
#![cfg_attr(docsrs, feature(doc_cfg))]
#![no_std]

#[cfg(feature = "alloc")]
extern crate alloc;

#[cfg(any(feature = "std", test))]
extern crate std;

/// Implements `Quiddity::__describe` (spec 6.8) with the `identity`
/// feature: `describe!(|cx| kind)`, with `Kd` naming `identity::Kind` and
/// `X` the module `identity::export`.
macro_rules! describe {
    (|$cx:ident| $body:expr) => {
        #[cfg(feature = "identity")]
        fn __describe(
            $cx: &mut crate::identity::export::Exporter,
        ) -> ::core::result::Result<crate::identity::Kind, crate::identity::ExportError> {
            #[allow(unused_imports)]
            use crate::identity::{Kind as Kd, export as X};
            #[allow(unused_variables)]
            let $cx = $cx;
            ::core::result::Result::Ok($body)
        }
    };
}

/// Implements `Quiddity::__encode_entries` (D21) with the `alloc` feature,
/// and marks the type flattenable: `flatten!(|this, m| body)`, with `this`
/// bound to `self` and `m` to the `Flattened` map.
#[cfg(feature = "alloc")]
macro_rules! flatten {
    (|$this:ident, $m:ident| $body:expr) => {
        #[cfg(feature = "alloc")]
        const __FLATTENABLE: bool = true;

        #[cfg(feature = "alloc")]
        fn __encode_entries(&self, $m: &mut crate::flatten::Flattened<'_>) {
            let $this = self;
            $body
        }
    };
}

mod digest;
#[cfg(all(doctest, feature = "derive"))]
mod doctests;
/// The README's examples, as doctests.
#[cfg(all(doctest, feature = "std", feature = "derive"))]
#[doc = include_str!("../README.md")]
pub struct ReadmeDoctests;
mod empty;
mod encoder;
mod features;
#[cfg(feature = "alloc")]
mod flatten;
#[cfg(feature = "identity")]
pub mod identity;
mod impls;
mod number;
mod quantize;
#[cfg(feature = "serde")]
pub mod serde;

#[cfg(all(test, feature = "alloc"))]
mod vectors;

pub use digest::{Digest, Digest64, Digest128, ParseDigestError};
pub use encoder::{BytesStream, Encoder, FieldsEncoder, ListEncoder, StrStream};
#[cfg(feature = "alloc")]
pub use encoder::{MapEncoder, SetEncoder};
pub use number::Integer;

#[cfg(feature = "derive")]
pub use quiddity_derive::*;

/// A marker type naming an identity: one choice of which fields a type's
/// digest covers, and how they are encoded. Implement it with
/// `#[derive(IdentityMarker)]`.
pub trait IdentityMarker: 'static {}

/// The primary identity: the one [`QuiddityExt::quiddity`] uses.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Primary;

impl IdentityMarker for Primary {}

/// How a value is encoded under identity `I`. Implemented by
/// `#[derive(Quiddity)]`, or by hand.
pub trait Quiddity<I: IdentityMarker = Primary> {
    /// The domain this type's digests are in (spec 4.2), if any.
    const DOMAIN: Option<&'static str> = None;

    /// Writes this value's encoding: exactly one value.
    fn encode(&self, e: &mut Encoder);

    /// Whether a field of this type is left out of a derived type's
    /// encoding, as if skipped. `true` only for `PhantomData`.
    #[doc(hidden)]
    const ABSENT_AS_FIELD: bool = false;

    /// Whether a field of this type can be flattened (D21): its members
    /// written into the containing struct's map by `__encode_entries`.
    /// `true` for structs with named fields, enums, maps, and `Option`s and
    /// pointers to them; checked when deriving a type with a flattened field.
    #[cfg(feature = "alloc")]
    #[doc(hidden)]
    const __FLATTENABLE: bool = false;

    /// Writes this value's members into the map of the struct containing
    /// it, for `flatten` (D21).
    #[cfg(feature = "alloc")]
    #[doc(hidden)]
    fn __encode_entries(&self, m: &mut flatten::Flattened<'_>) {
        m.cannot_flatten(core::any::type_name::<Self>())
    }

    /// The identity document (spec 5) describing this type's encoding
    /// under `I`, for values in the JSON form serde writes (spec 6.8).
    ///
    /// The name says that this builds a structure rather than returning
    /// one. It fails if the encoding uses something an identity document
    /// cannot express, such as a `with` function, or reaches a type whose
    /// implementation does not describe itself.
    #[cfg(feature = "identity")]
    fn construct_identity() -> Result<identity::Identity, identity::ExportError> {
        identity::export::construct::<Self, I>()
    }

    /// Describes this type's encoding under `I` as a kind (spec 6.8). Used
    /// by [`Quiddity::construct_identity`]; implementations that do not
    /// override it cannot be exported.
    #[cfg(feature = "identity")]
    #[doc(hidden)]
    fn __describe(
        cx: &mut identity::export::Exporter,
    ) -> Result<identity::Kind, identity::ExportError> {
        Err(cx.unsupported_type(core::any::type_name::<Self>()))
    }
}

/// The methods that compute digests. Implemented for every type; available
/// on any type that implements [`Quiddity`].
pub trait QuiddityExt {
    /// The digest of this value under its primary identity.
    fn quiddity(&self) -> Digest
    where
        Self: Quiddity<Primary>,
    {
        self.quiddity_as::<Primary>()
    }

    /// The digest of this value under identity `I`.
    fn quiddity_as<I: IdentityMarker>(&self) -> Digest
    where
        Self: Quiddity<I>,
    {
        let mut e = Encoder::new();
        <Self as Quiddity<I>>::encode(self, &mut e);
        let structural = e.finish();
        match <Self as Quiddity<I>>::DOMAIN {
            None => structural,
            Some(label) => structural.in_domain(label),
        }
    }
}

impl<T: ?Sized> QuiddityExt for T {}

/// Used by the derive's expansion. Not part of the public API.
#[doc(hidden)]
pub mod __private {
    pub use crate::empty::{IsEmpty, Nullable};
    #[cfg(feature = "alloc")]
    pub use crate::flatten::{Flattened, flattened};
    pub use crate::quantize::{Decimals, Quantize};

    #[cfg(feature = "identity")]
    pub use alloc::vec;

    #[cfg(feature = "alloc")]
    pub use crate::impls::alloc_impls::encode_set;

    #[cfg(feature = "serde")]
    pub use crate::serde::encode_or_panic as encode_serde;
    #[cfg(feature = "serde")]
    pub use crate::serde::encode_with_or_panic as encode_serialize_with;
}

/// Used by the derive's expansion: its contents with the `identity`
/// feature, and nothing without it.
#[doc(hidden)]
#[cfg(feature = "identity")]
#[macro_export]
macro_rules! __if_identity {
    ($($tokens:tt)*) => { $($tokens)* };
}

/// Used by the derive's expansion: its contents with the `identity`
/// feature, and nothing without it.
#[doc(hidden)]
#[cfg(not(feature = "identity"))]
#[macro_export]
macro_rules! __if_identity {
    ($($tokens:tt)*) => {};
}

/// Used by the derive's expansion: its contents with the `alloc` feature,
/// and nothing without it (the flattening members of derived types).
#[doc(hidden)]
#[cfg(feature = "alloc")]
#[macro_export]
macro_rules! __if_alloc {
    ($($tokens:tt)*) => { $($tokens)* };
}

/// Used by the derive's expansion: its contents with the `alloc` feature,
/// and nothing without it (the flattening members of derived types).
#[doc(hidden)]
#[cfg(not(feature = "alloc"))]
#[macro_export]
macro_rules! __if_alloc {
    ($($tokens:tt)*) => {};
}

/// Used by the derive's expansion for `flatten`: the map of the struct
/// being written.
#[doc(hidden)]
#[cfg(feature = "alloc")]
#[macro_export]
macro_rules! __flattened {
    ($e:expr, $type_name:expr) => {
        $crate::__private::flattened($e, $type_name)
    };
}

/// Used by the derive's expansion for `flatten`.
#[doc(hidden)]
#[cfg(not(feature = "alloc"))]
#[macro_export]
macro_rules! __flattened {
    ($e:expr, $type_name:expr) => {
        ::core::compile_error!("`flatten` needs the `alloc` feature of `quiddity`")
    };
}

/// Used by the derive's expansion for `encode_as = set`.
#[doc(hidden)]
#[cfg(feature = "alloc")]
#[macro_export]
macro_rules! __encode_set {
    ($value:expr, $e:expr) => {
        $crate::__private::encode_set($value, $e)
    };
}

/// Used by the derive's expansion for `encode_as = set`.
#[doc(hidden)]
#[cfg(not(feature = "alloc"))]
#[macro_export]
macro_rules! __encode_set {
    ($value:expr, $e:expr) => {
        ::core::compile_error!("`encode_as = set` needs the `alloc` feature of `quiddity`")
    };
}

/// Used by the derive's expansion for serde's `serialize_with` and `with`
/// (`serde_attributes`, D21): `$f` is a closure calling the function with
/// the bridge's serializer.
#[doc(hidden)]
#[cfg(feature = "serde")]
#[macro_export]
macro_rules! __encode_serialize_with {
    ($e:expr, $f:expr) => {
        $crate::__private::encode_serialize_with($e, $f)
    };
}

/// Used by the derive's expansion for serde's `serialize_with` and `with`.
#[doc(hidden)]
#[cfg(not(feature = "serde"))]
#[macro_export]
macro_rules! __encode_serialize_with {
    ($e:expr, $f:expr) => {
        ::core::compile_error!(
            "serde's `serialize_with` and `with` need the `serde` feature of `quiddity`"
        )
    };
}

/// Used by the derive's expansion for `encode_as = serde`.
#[doc(hidden)]
#[cfg(feature = "serde")]
#[macro_export]
macro_rules! __encode_serde {
    ($value:expr, $e:expr) => {
        $crate::__private::encode_serde($value, $e)
    };
}

/// Used by the derive's expansion for `encode_as = serde`.
#[doc(hidden)]
#[cfg(not(feature = "serde"))]
#[macro_export]
macro_rules! __encode_serde {
    ($value:expr, $e:expr) => {
        ::core::compile_error!("`encode_as = serde` needs the `serde` feature of `quiddity`")
    };
}
