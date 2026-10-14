//! The derive's compile errors (spec 6.3–6.5).
//!
//! The expected messages are those of a build without the `serde` and
//! `serde_json` features: `serde` turns one of the errors into a success,
//! and `serde_json` adds impls to the compiler's lists of suggestions. In a
//! workspace build, the `arbitrary-precision` test crate turns on
//! `serde_json`, so run `cargo test` in this package to check them.

#![cfg(all(
    feature = "derive",
    not(feature = "serde"),
    not(feature = "serde_json")
))]

#[test]
fn compile_fail() {
    trybuild::TestCases::new().compile_fail("tests/ui/*.rs");
}
