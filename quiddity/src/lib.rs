//! Stable digests of values over the fields you choose.
//!
//! See the README for an overview, and `QUIDDITY_SPEC.md` for the encoding.

#![no_std]

#[cfg(feature = "alloc")]
extern crate alloc;

#[cfg(feature = "std")]
extern crate std;

#[cfg(feature = "derive")]
pub use quiddity_derive::*;
