//! Stable digests of values over the fields you choose.
//!
//! See the README for an overview, and `QUIDDITY_SPEC.md` for the encoding.

#![no_std]

#[cfg(feature = "alloc")]
extern crate alloc;

#[cfg(any(feature = "std", test))]
extern crate std;

mod digest;
mod encoder;
mod features;
mod impls;
mod number;
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
    /// Implemented only for `Option<T>`, so that `none_as_missing` on any
    /// other type fails to compile.
    #[diagnostic::on_unimplemented(
        message = "`none_as_missing` applies only to `Option` fields, not `{Self}`",
        label = "not an `Option`"
    )]
    pub trait OptionLike {
        fn is_none(&self) -> bool;
    }

    impl<T> OptionLike for Option<T> {
        fn is_none(&self) -> bool {
            Option::is_none(self)
        }
    }

    #[cfg(feature = "serde")]
    pub use crate::serde::encode_or_panic as encode_serde;
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
