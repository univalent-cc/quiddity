//! The derive's compile errors (spec 6.3–6.5).

#![cfg(all(feature = "derive", not(feature = "serde")))]

#[test]
fn compile_fail() {
    trybuild::TestCases::new().compile_fail("tests/ui/*.rs");
}
