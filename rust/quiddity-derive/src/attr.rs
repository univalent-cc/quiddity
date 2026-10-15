//! Parsing `#[quiddity(…)]` attributes, each of the form
//! `name[(Identity, …)] [= value]` (spec 6.5).

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

/// The key identifying an identity in scope lists: its type name, the last
/// segment of its path, with any generic arguments. A derive macro cannot
/// resolve paths, so `Meaning` and `crate::Meaning` are matched by name; the
/// identities a type declares must therefore have distinct names.
pub fn identity_key(path: &Path) -> String {
    path.segments.last().unwrap().to_token_stream().to_string()
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

    /// Sets the value for the identity `key`, as serde's attributes do in
    /// the `serde_attributes` mode (D21).
    pub fn set_for(&mut self, key: &str, value: T) {
        self.by.retain(|(k, _)| k != key);
        self.by.push((key.to_owned(), value));
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

/// An attribute's value, without the invisible group that wraps a value
/// passed in through a `macro_rules!` fragment.
fn value(item: &Item) -> Option<&Expr> {
    let mut e = item.value.as_ref()?;
    while let Expr::Group(g) = e {
        e = &g.expr;
    }
    Some(e)
}

fn flag(item: &Item) -> syn::Result<bool> {
    match value(item) {
        None => Ok(true),
        Some(Expr::Lit(ExprLit {
            lit: Lit::Bool(b), ..
        })) => Ok(b.value),
        Some(other) => Err(syn::Error::new_spanned(other, "expected `true` or `false`")),
    }
}

fn string(item: &Item) -> syn::Result<LitStr> {
    match value(item) {
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

/// An integer literal, possibly negated.
fn integer(item: &Item) -> syn::Result<i128> {
    match value(item) {
        Some(Expr::Lit(ExprLit {
            lit: Lit::Int(i), ..
        })) => i.base10_parse(),
        Some(Expr::Unary(syn::ExprUnary {
            op: syn::UnOp::Neg(_),
            expr,
            ..
        })) => match &**expr {
            Expr::Lit(ExprLit {
                lit: Lit::Int(i), ..
            }) => i.base10_parse::<i128>().map(|n| -n),
            other => Err(syn::Error::new_spanned(other, "expected an integer")),
        },
        Some(other) => Err(syn::Error::new_spanned(other, "expected an integer")),
        None => Err(syn::Error::new(
            item.name.span(),
            format!("expected `{} = <integer>`", item.name),
        )),
    }
}

fn path(item: &Item) -> syn::Result<Path> {
    match value(item) {
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
    Set,
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
        Some("set") => EncodeAs::Set,
        _ => {
            return Err(syn::Error::new_spanned(
                p,
                "expected one of `map`, `list`, `set`, `string`, `int`, `display`, `serde`",
            ));
        }
    };
    Ok((shape, item.name.span()))
}

/// A case convention for `rename_all`, with serde's names and rules.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RenameRule {
    Lower,
    Upper,
    Pascal,
    Camel,
    Snake,
    ScreamingSnake,
    Kebab,
    ScreamingKebab,
}

const RENAME_RULES: &[(&str, RenameRule)] = &[
    ("lowercase", RenameRule::Lower),
    ("UPPERCASE", RenameRule::Upper),
    ("PascalCase", RenameRule::Pascal),
    ("camelCase", RenameRule::Camel),
    ("snake_case", RenameRule::Snake),
    ("SCREAMING_SNAKE_CASE", RenameRule::ScreamingSnake),
    ("kebab-case", RenameRule::Kebab),
    ("SCREAMING-KEBAB-CASE", RenameRule::ScreamingKebab),
];

impl RenameRule {
    pub fn parse(lit: &LitStr) -> syn::Result<Self> {
        let value = lit.value();
        RENAME_RULES
            .iter()
            .find(|(name, _)| *name == value)
            .map(|(_, rule)| *rule)
            .ok_or_else(|| {
                let names: Vec<String> =
                    RENAME_RULES.iter().map(|(n, _)| format!("{n:?}")).collect();
                syn::Error::new_spanned(lit, format!("expected one of {}", names.join(", ")))
            })
    }

    /// A field name, written in snake_case, in this convention.
    pub fn apply_to_field(self, field: &str) -> String {
        match self {
            RenameRule::Lower | RenameRule::Snake => field.to_owned(),
            RenameRule::Upper | RenameRule::ScreamingSnake => field.to_ascii_uppercase(),
            RenameRule::Pascal => {
                let mut pascal = String::new();
                let mut capitalize = true;
                for ch in field.chars() {
                    if ch == '_' {
                        capitalize = true;
                    } else if capitalize {
                        pascal.push(ch.to_ascii_uppercase());
                        capitalize = false;
                    } else {
                        pascal.push(ch);
                    }
                }
                pascal
            }
            RenameRule::Camel => lower_first(&RenameRule::Pascal.apply_to_field(field)),
            RenameRule::Kebab => field.replace('_', "-"),
            RenameRule::ScreamingKebab => field.to_ascii_uppercase().replace('_', "-"),
        }
    }

    /// A variant name, written in PascalCase, in this convention.
    pub fn apply_to_variant(self, variant: &str) -> String {
        match self {
            RenameRule::Pascal => variant.to_owned(),
            RenameRule::Lower => variant.to_ascii_lowercase(),
            RenameRule::Upper => variant.to_ascii_uppercase(),
            RenameRule::Camel => lower_first(variant),
            RenameRule::Snake => {
                let mut snake = String::new();
                for (i, ch) in variant.char_indices() {
                    if i > 0 && ch.is_uppercase() {
                        snake.push('_');
                    }
                    snake.push(ch.to_ascii_lowercase());
                }
                snake
            }
            RenameRule::ScreamingSnake => RenameRule::Snake
                .apply_to_variant(variant)
                .to_ascii_uppercase(),
            RenameRule::Kebab => RenameRule::Snake
                .apply_to_variant(variant)
                .replace('_', "-"),
            RenameRule::ScreamingKebab => RenameRule::ScreamingSnake
                .apply_to_variant(variant)
                .replace('_', "-"),
        }
    }
}

fn lower_first(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        Some(first) => first.to_ascii_lowercase().to_string() + chars.as_str(),
        None => String::new(),
    }
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
    pub rename_all: Scoped<RenameRule>,
    /// Internal or adjacent tagging of an enum: the tag member's name.
    pub tag: Scoped<LitStr>,
    /// Adjacent tagging of an enum: the content member's name.
    pub content: Scoped<LitStr>,
    pub untagged: Scoped<bool>,
    /// The identities whose attributes are serde's (D21).
    pub serde_attributes: Scoped<bool>,
    /// Set from serde only: `rename_all_fields`, the convention for the
    /// fields of every struct variant.
    pub rename_all_fields: Scoped<RenameRule>,
    /// Set from serde only: a struct's `tag`, and the name it writes there.
    pub struct_tag: Scoped<(LitStr, String)>,
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
                    return Err(syn::Error::new_spanned(
                        p,
                        format!(
                            "two identities named `{}`: identities are matched by name \
                             in attribute lists, so their names must differ",
                            identity_key(p)
                        ),
                    ));
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
                "encode_as" => {
                    let (shape, span) = encode_as(item)?;
                    if shape == EncodeAs::Set {
                        return Err(syn::Error::new(
                            span,
                            "`encode_as = set` applies to fields, not types",
                        ));
                    }
                    out.encode_as.set(item, (shape, span), &ids)?;
                }
                "rename_all" => {
                    let rule = RenameRule::parse(&string(item)?)?;
                    out.rename_all.set(item, rule, &ids)?;
                }
                "tag" => {
                    let s = string(item)?;
                    if s.value().is_empty() {
                        return Err(syn::Error::new_spanned(s, "a tag must not be empty"));
                    }
                    out.tag.set(item, s, &ids)?;
                }
                "content" => {
                    let s = string(item)?;
                    if s.value().is_empty() {
                        return Err(syn::Error::new_spanned(
                            s,
                            "a content name must not be empty",
                        ));
                    }
                    out.content.set(item, s, &ids)?;
                }
                "untagged" => out.untagged.set(item, flag(item)?, &ids)?,
                "serde_attributes" => out.serde_attributes.set(item, flag(item)?, &ids)?,
                _ => return Err(unknown(item, "type")),
            }
        }
        Ok(out)
    }

    /// The keys of the identities in the `serde_attributes` mode.
    pub fn serde_keys(&self) -> Vec<String> {
        self.keys()
            .into_iter()
            .filter(|k| self.serde_attributes.get(k).copied().unwrap_or(false))
            .collect()
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
    pub omit_none: Scoped<bool>,
    pub omit_empty: Scoped<bool>,
    pub omit_if: Scoped<Path>,
    pub quantize: Scoped<u64>,
    pub decimals: Scoped<i32>,
    pub encode_as: Scoped<(EncodeAs, Span)>,
    /// The field's members are written into the containing map (D21).
    pub flatten: Scoped<bool>,
    /// Set from serde only: a `serialize_with` function (or `with`'s
    /// `serialize`), applied through the serde bridge.
    pub serialize_with: Scoped<Path>,
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
                    let p = if identity_key(&p) == PRIMARY {
                        syn::parse_quote!(::quiddity::Primary)
                    } else {
                        p
                    };
                    out.identity.set(item, p, ids)?;
                }
                "with" => out.with.set(item, path(item)?, ids)?,
                "flatten" => out.flatten.set(item, flag(item)?, ids)?,
                "omit_none" => out.omit_none.set(item, flag(item)?, ids)?,
                "omit_empty" => out.omit_empty.set(item, flag(item)?, ids)?,
                "omit_if" => out.omit_if.set(item, path(item)?, ids)?,
                "quantize" => {
                    let q = integer(item)?;
                    let q = u64::try_from(q).ok().filter(|q| *q >= 1).ok_or_else(|| {
                        syn::Error::new(item.name.span(), "`quantize` must be a positive integer")
                    })?;
                    out.quantize.set(item, q, ids)?;
                }
                "decimals" => {
                    let d = integer(item)?;
                    let d = i32::try_from(d)
                        .ok()
                        .filter(|d| (-300..=300).contains(d))
                        .ok_or_else(|| {
                            syn::Error::new(item.name.span(), "`decimals` must be from -300 to 300")
                        })?;
                    out.decimals.set(item, d, ids)?;
                }
                "encode_as" => {
                    let (shape, span) = encode_as(item)?;
                    if !matches!(shape, EncodeAs::Display | EncodeAs::Serde | EncodeAs::Set) {
                        return Err(syn::Error::new(
                            span,
                            "on a field, `encode_as` must be `set`, `display` or `serde`",
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
    /// Set from serde only: the variant's `rename_all`, for its fields.
    pub fields_rename_all: Scoped<RenameRule>,
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
