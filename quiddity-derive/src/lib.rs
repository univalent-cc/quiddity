//! Derive macros for `quiddity`. Use them through the `quiddity` crate, which
//! re-exports them with its `derive` feature (on by default).

use proc_macro::TokenStream;
use quote::quote_spanned;
use syn::{DeriveInput, parse_macro_input};

#[proc_macro_derive(Quiddity, attributes(quiddity))]
pub fn derive_quiddity(input: TokenStream) -> TokenStream {
    not_yet_implemented(parse_macro_input!(input as DeriveInput), "Quiddity")
}

#[proc_macro_derive(IdentityMarker)]
pub fn derive_identity_marker(input: TokenStream) -> TokenStream {
    not_yet_implemented(parse_macro_input!(input as DeriveInput), "IdentityMarker")
}

fn not_yet_implemented(input: DeriveInput, name: &str) -> TokenStream {
    let message = format!("`#[derive({name})]` is not implemented yet");
    quote_spanned!(input.ident.span()=> ::core::compile_error!(#message);).into()
}
