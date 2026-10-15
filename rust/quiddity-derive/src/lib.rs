//! Derive macros for `quiddity`. Use them through the `quiddity` crate, which
//! re-exports them with its `derive` feature (on by default).

mod attr;
mod expand;
mod serde_attrs;

use proc_macro::TokenStream;
use quote::quote;
use syn::{DeriveInput, parse_macro_input};

/// Implements `Quiddity` for the primary identity and each identity listed
/// in `#[quiddity(identities(…))]`. See the `quiddity` crate's README for
/// the attributes.
#[proc_macro_derive(Quiddity, attributes(quiddity))]
pub fn derive_quiddity(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    expand::derive(input)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

/// Implements `IdentityMarker`, making the type an identity.
#[proc_macro_derive(IdentityMarker)]
pub fn derive_identity_marker(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    if input.ident == attr::PRIMARY {
        return syn::Error::new_spanned(
            &input.ident,
            "a marker type named `Primary` is not allowed: it would be confused with \
             `quiddity::Primary` in attribute lists",
        )
        .into_compile_error()
        .into();
    }
    let name = &input.ident;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();
    quote! {
        #[automatically_derived]
        impl #impl_generics ::quiddity::IdentityMarker for #name #ty_generics #where_clause {}
    }
    .into()
}
