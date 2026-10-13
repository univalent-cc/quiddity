//! Parsing `#[quiddity(…)]` attributes, each of the form
//! `name[(Identity, …)] [= value]` (spec 5.5).

use proc_macro2::Span;
use quote::ToTokens;
use syn::parse::{Parse, ParseStream};
use syn::punctuated::Punctuated;
use syn::{Attribute, Expr, ExprLit, Ident, Lit, LitStr, Path, Token, parenthesized};

/// The key of the primary identity in scope lists.
pub const PRIMARY: &str = "Primary";

/// One item in a `#[quiddity(…)]` list.
struct Item {
    name: Ident,
    /// The parenthesized list after the name: identities for a scoped
    /// attribute, or the list of `identities(…)`.
    parens: Option<Vec<Path>>,
    value: Option<Expr>,
}

impl Parse for Item {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let name: Ident = input.parse()?;
        let parens = if input.peek(syn::token::Paren) {
            let content;
            parenthesized!(content in input);
            let paths = Punctuated::<Path, Token![,]>::parse_terminated(&content)?;
            Some(paths.into_iter().collect())
        } else {
            None
        };
        let value = if input.peek(Token![=]) {
            input.parse::<Token![=]>()?;
            Some(input.parse()?)
        } else {
            None
        };
        Ok(Item {
            name,
            parens,
            value,
        })
    }
}

fn items(attrs: &[Attribute]) -> syn::Result<Vec<Item>> {
    let mut out = Vec::new();
    for attr in attrs.iter().filter(|a| a.path().is_ident("quiddity")) {
        let list = attr.parse_args_with(Punctuated::<Item, Token![,]>::parse_terminated)?;
        out.extend(list);
    }
    Ok(out)
}

/// The key identifying an identity path, for comparing scope lists with
/// the declared identities.
pub fn identity_key(path: &Path) -> String {
    if path.is_ident(PRIMARY) {
        PRIMARY.to_owned()
    } else {
        path.to_token_stream().to_string()
    }
}

/// An attribute's values: an unscoped one, and scoped ones.
pub struct Scoped<T> {
    all: Option<T>,
    by: Vec<(String, T)>,
}

impl<T> Default for Scoped<T> {
    fn default() -> Self {
        Scoped {
            all: None,
            by: Vec::new(),
        }
    }
}

impl<T: Clone> Scoped<T> {
    /// The value for the identity `key`: scoped if given, else unscoped.
    pub fn get(&self, key: &str) -> Option<&T> {
        self.by
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v)
            .or(self.all.as_ref())
    }

    /// Whether the attribute is written at all.
    pub fn is_set(&self) -> bool {
        self.all.is_some() || !self.by.is_empty()
    }

    fn set(&mut self, item: &Item, value: T, ids: &[String]) -> syn::Result<()> {
        let Some(paths) = &item.parens else {
            if self.all.is_some() {
                return Err(syn::Error::new(item.name.span(), "duplicate attribute"));
            }
            self.all = Some(value);
            return Ok(());
        };
        if paths.is_empty() {
            return Err(syn::Error::new(item.name.span(), "empty identity list"));
        }
        for path in paths {
            let key = identity_key(path);
            if !ids.contains(&key) {
                return Err(syn::Error::new_spanned(
                    path,
                    format!("identity `{key}` is not declared in this type's `identities(…)`"),
                ));
            }
            if self.by.iter().any(|(k, _)| *k == key) {
                return Err(syn::Error::new_spanned(
                    path,
                    format!("duplicate attribute for identity `{key}`"),
                ));
            }
            self.by.push((key, value.clone()));
        }
        Ok(())
    }
}

fn flag(item: &Item) -> syn::Result<bool> {
    match &item.value {
        None => Ok(true),
        Some(Expr::Lit(ExprLit {
            lit: Lit::Bool(b), ..
        })) => Ok(b.value),
        Some(other) => Err(syn::Error::new_spanned(other, "expected `true` or `false`")),
    }
}

fn string(item: &Item) -> syn::Result<LitStr> {
    match &item.value {
        Some(Expr::Lit(ExprLit {
            lit: Lit::Str(s), ..
        })) => Ok(s.clone()),
        Some(other) => Err(syn::Error::new_spanned(other, "expected a string literal")),
        None => Err(syn::Error::new(
            item.name.span(),
            format!("expected `{} = \"…\"`", item.name),
        )),
    }
}

fn path(item: &Item) -> syn::Result<Path> {
    match &item.value {
        Some(Expr::Path(p)) if p.qself.is_none() => Ok(p.path.clone()),
        Some(other) => Err(syn::Error::new_spanned(other, "expected a path")),
        None => Err(syn::Error::new(
            item.name.span(),
            format!("expected `{} = <path>`", item.name),
        )),
    }
}

/// The values of `encode_as`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum EncodeAs {
    Map,
    List,
    String,
    Int,
    Display,
    Serde,
}

fn encode_as(item: &Item) -> syn::Result<(EncodeAs, Span)> {
    let p = path(item)?;
    let shape = match p.get_ident().map(|i| i.to_string()).as_deref() {
        Some("map") => EncodeAs::Map,
        Some("list") => EncodeAs::List,
        Some("string") => EncodeAs::String,
        Some("int") => EncodeAs::Int,
        Some("display") => EncodeAs::Display,
        Some("serde") => EncodeAs::Serde,
        _ => {
            return Err(syn::Error::new_spanned(
                p,
                "expected one of `map`, `list`, `string`, `int`, `display`, `serde`",
            ));
        }
    };
    Ok((shape, item.name.span()))
}

fn unknown(item: &Item, what: &str) -> syn::Error {
    syn::Error::new(
        item.name.span(),
        format!("unknown {what} attribute `{}`", item.name),
    )
}

/// Attributes on a struct or enum.
#[derive(Default)]
pub struct TypeAttrs {
    pub identities: Vec<Path>,
    pub domain: Scoped<LitStr>,
    pub transparent: Scoped<bool>,
    pub encode_as: Scoped<(EncodeAs, Span)>,
}

impl TypeAttrs {
    pub fn parse(attrs: &[Attribute]) -> syn::Result<Self> {
        let items = items(attrs)?;
        let mut out = TypeAttrs::default();
        // Identities first, so that scope lists can be checked against them.
        for item in items.iter().filter(|i| i.name == "identities") {
            if item.value.is_some() {
                return Err(syn::Error::new(
                    item.name.span(),
                    "expected `identities(A, B, …)`",
                ));
            }
            for p in item.parens.iter().flatten() {
                let last = &p.segments.last().unwrap().ident;
                if last == PRIMARY {
                    return Err(syn::Error::new_spanned(
                        p,
                        "the primary identity is always implemented; \
                         a marker type named `Primary` is not allowed",
                    ));
                }
                if out
                    .identities
                    .iter()
                    .any(|q| identity_key(q) == identity_key(p))
                {
                    return Err(syn::Error::new_spanned(p, "duplicate identity"));
                }
                out.identities.push(p.clone());
            }
        }
        let ids = out.keys();
        for item in &items {
            match item.name.to_string().as_str() {
                "identities" => {}
                "domain" => {
                    let s = string(item)?;
                    if s.value().is_empty() {
                        return Err(syn::Error::new_spanned(s, "a domain must not be empty"));
                    }
                    out.domain.set(item, s, &ids)?;
                }
                "transparent" => out.transparent.set(item, flag(item)?, &ids)?,
                "encode_as" => out.encode_as.set(item, encode_as(item)?, &ids)?,
                _ => return Err(unknown(item, "type")),
            }
        }
        Ok(out)
    }

    /// The keys of every identity the type implements, primary first.
    pub fn keys(&self) -> Vec<String> {
        let mut keys = vec![PRIMARY.to_owned()];
        keys.extend(self.identities.iter().map(identity_key));
        keys
    }
}

/// Attributes on a field.
#[derive(Default)]
pub struct FieldAttrs {
    pub skip: Scoped<bool>,
    pub rename: Scoped<LitStr>,
    pub identity: Scoped<Path>,
    pub with: Scoped<Path>,
    pub none_as_missing: Scoped<bool>,
    pub omit_if: Scoped<Path>,
    pub encode_as: Scoped<(EncodeAs, Span)>,
}

impl FieldAttrs {
    pub fn parse(attrs: &[Attribute], ids: &[String]) -> syn::Result<Self> {
        let mut out = FieldAttrs::default();
        for item in &items(attrs)? {
            match item.name.to_string().as_str() {
                "skip" => out.skip.set(item, flag(item)?, ids)?,
                "rename" => out.rename.set(item, string(item)?, ids)?,
                "identity" => {
                    let p = path(item)?;
                    let p = if p.is_ident(PRIMARY) {
                        syn::parse_quote!(::quiddity::Primary)
                    } else {
                        p
                    };
                    out.identity.set(item, p, ids)?;
                }
                "with" => out.with.set(item, path(item)?, ids)?,
                "none_as_missing" => out.none_as_missing.set(item, flag(item)?, ids)?,
                "omit_if" => out.omit_if.set(item, path(item)?, ids)?,
                "encode_as" => {
                    let (shape, span) = encode_as(item)?;
                    if !matches!(shape, EncodeAs::Display | EncodeAs::Serde) {
                        return Err(syn::Error::new(
                            span,
                            "on a field, `encode_as` must be `display` or `serde`",
                        ));
                    }
                    out.encode_as.set(item, (shape, span), ids)?;
                }
                _ => return Err(unknown(item, "field")),
            }
        }
        Ok(out)
    }
}

/// Attributes on an enum variant.
#[derive(Default)]
pub struct VariantAttrs {
    pub rename: Scoped<LitStr>,
}

impl VariantAttrs {
    pub fn parse(attrs: &[Attribute], ids: &[String]) -> syn::Result<Self> {
        let mut out = VariantAttrs::default();
        for item in &items(attrs)? {
            match item.name.to_string().as_str() {
                "rename" => out.rename.set(item, string(item)?, ids)?,
                _ => {
                    return Err(syn::Error::new(
                        item.name.span(),
                        format!(
                            "unknown variant attribute `{}`; only `rename` applies",
                            item.name
                        ),
                    ));
                }
            }
        }
        Ok(out)
    }
}
