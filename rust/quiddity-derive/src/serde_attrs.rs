//! `#[quiddity(serde_attributes)]` (D21): serde's attributes read as their
//! quiddity equivalents, for the identities in the mode. A quiddity
//! attribute with a serde counterpart is an error for those identities.

use proc_macro2::Span;
use syn::meta::ParseNestedMeta;
use syn::spanned::Spanned;
use syn::{Attribute, LitStr, Path, Token};

use crate::attr::{EncodeAs, FieldAttrs, RenameRule, TypeAttrs, VariantAttrs};

/// What serde says about a type.
#[derive(Default)]
struct SerdeType {
    rename: Option<LitStr>,
    rename_all: Option<RenameRule>,
    rename_all_fields: Option<RenameRule>,
    tag: Option<LitStr>,
    content: Option<LitStr>,
    untagged: bool,
    transparent: bool,
    into: Option<Span>,
}

/// What serde says about a field.
#[derive(Default)]
struct SerdeField {
    rename: Option<LitStr>,
    skip: bool,
    skip_if: Option<LitStr>,
    serialize_with: Option<Path>,
    flatten: bool,
}

/// What serde says about a variant.
#[derive(Default)]
struct SerdeVariant {
    rename: Option<LitStr>,
    rename_all: Option<RenameRule>,
    /// A variant serialized in a way the derive does not follow
    /// (`untagged` on one variant, `serialize_with`): the whole type is
    /// then digested through its `Serialize` impl.
    through_serde: Option<(Span, &'static str)>,
}

/// Serde's attributes that do not affect serialization.
const TYPE_IGNORED: &[&str] = &[
    "default",
    "deny_unknown_fields",
    "bound",
    "crate",
    "expecting",
    "from",
    "try_from",
    "variant_identifier",
    "field_identifier",
];
const FIELD_IGNORED: &[&str] = &[
    "alias",
    "default",
    "skip_deserializing",
    "deserialize_with",
    "borrow",
    "bound",
];
/// `skip` and `skip_serializing` on a variant make serializing it fail; a
/// value without a wire form has nothing to agree with, so they are ignored.
const VARIANT_IGNORED: &[&str] = &[
    "alias",
    "skip",
    "skip_serializing",
    "skip_deserializing",
    "deserialize_with",
    "other",
    "borrow",
    "bound",
];

/// Calls `f` with the name and contents of each item in `#[serde(…)]`.
fn each(
    attrs: &[Attribute],
    mut f: impl FnMut(&str, ParseNestedMeta) -> syn::Result<()>,
) -> syn::Result<()> {
    for attr in attrs.iter().filter(|a| a.path().is_ident("serde")) {
        attr.parse_nested_meta(|meta| {
            let name = match meta.path.get_ident() {
                Some(i) => i.to_string(),
                None => return Err(meta.error("unknown serde attribute")),
            };
            f(&name, meta)
        })?;
    }
    Ok(())
}

/// Consumes an item's value or list, which is ignored.
fn skip(meta: &ParseNestedMeta) -> syn::Result<()> {
    if meta.input.peek(Token![=]) {
        meta.value()?.parse::<syn::Expr>()?;
    } else if meta.input.peek(syn::token::Paren) {
        meta.parse_nested_meta(|inner| skip(&inner))?;
    }
    Ok(())
}

/// The serialize side of `name = "…"` or `name(serialize = "…", deserialize = "…")`.
fn serialize_side(meta: &ParseNestedMeta) -> syn::Result<Option<LitStr>> {
    if meta.input.peek(Token![=]) {
        return Ok(Some(meta.value()?.parse()?));
    }
    let mut out = None;
    meta.parse_nested_meta(|inner| {
        if inner.path.is_ident("serialize") {
            out = Some(inner.value()?.parse()?);
            Ok(())
        } else if inner.path.is_ident("deserialize") {
            skip(&inner)
        } else {
            Err(inner.error("expected `serialize` or `deserialize`"))
        }
    })?;
    Ok(out)
}

fn rule(meta: &ParseNestedMeta) -> syn::Result<Option<RenameRule>> {
    serialize_side(meta)?
        .map(|lit| RenameRule::parse(&lit))
        .transpose()
}

fn unknown(meta: &ParseNestedMeta, name: &str, on: &str) -> syn::Error {
    meta.error(format!(
        "unknown serde attribute `{name}` on a {on}: `serde_attributes` does not know how it \
         affects serialization, so it cannot follow it. This version of quiddity may be older \
         than the serde in use; please report it"
    ))
}

fn parse_type(attrs: &[Attribute], ident: &syn::Ident) -> syn::Result<SerdeType> {
    let mut t = SerdeType::default();
    each(attrs, |name, meta| {
        match name {
            // A container's name, which JSON writes only under a struct's
            // `tag`.
            "rename" => t.rename = serialize_side(&meta)?,
            "rename_all" => t.rename_all = rule(&meta)?,
            "rename_all_fields" => t.rename_all_fields = rule(&meta)?,
            "tag" => t.tag = Some(meta.value()?.parse()?),
            "content" => t.content = Some(meta.value()?.parse()?),
            "untagged" => t.untagged = true,
            "transparent" => t.transparent = true,
            "into" => {
                t.into = Some(meta.path.span());
                skip(&meta)?;
            }
            "remote" => {
                let other: LitStr = meta.value()?.parse()?;
                let other = other.value();
                return Err(meta.error(format!(
                    "`#[serde(remote = \"{other}\")]` is not supported with \
                     `#[quiddity(serde_attributes)]`: a remote derive serializes `{other}`, not \
                     `{ident}`, so `{ident}` has no serialized form to follow. Remove `Quiddity` \
                     from this derive: fields of type `{other}` marked \
                     `#[serde(with = \"{ident}\")]` are digested through it, with the `serde` \
                     feature. Or implement `Quiddity` for a wrapper of `{other}` by hand"
                )));
            }
            n if TYPE_IGNORED.contains(&n) => skip(&meta)?,
            n => return Err(unknown(&meta, n, "type")),
        }
        Ok(())
    })?;
    Ok(t)
}

fn parse_field(attrs: &[Attribute]) -> syn::Result<SerdeField> {
    let mut f = SerdeField::default();
    each(attrs, |name, meta| {
        match name {
            "rename" => f.rename = serialize_side(&meta)?,
            "skip" | "skip_serializing" => f.skip = true,
            "skip_serializing_if" => f.skip_if = Some(meta.value()?.parse()?),
            "serialize_with" => {
                let lit: LitStr = meta.value()?.parse()?;
                f.serialize_with = Some(lit.parse()?);
            }
            "with" => {
                let lit: LitStr = meta.value()?.parse()?;
                let mut path: Path = lit.parse()?;
                path.segments
                    .push(syn::Ident::new("serialize", lit.span()).into());
                f.serialize_with = Some(path);
            }
            "flatten" => f.flatten = true,
            "getter" => {
                return Err(meta.error(
                    "`getter` is only valid with `#[serde(remote)]`, which \
                     `serde_attributes` does not support",
                ));
            }
            n if FIELD_IGNORED.contains(&n) => skip(&meta)?,
            n => return Err(unknown(&meta, n, "field")),
        }
        Ok(())
    })?;
    Ok(f)
}

fn parse_variant(attrs: &[Attribute]) -> syn::Result<SerdeVariant> {
    let mut v = SerdeVariant::default();
    each(attrs, |name, meta| {
        match name {
            "rename" => v.rename = serialize_side(&meta)?,
            "rename_all" => v.rename_all = rule(&meta)?,
            "untagged" => v.through_serde = Some((meta.path.span(), "untagged")),
            "serialize_with" | "with" => {
                v.through_serde = Some((meta.path.span(), "serialize_with"));
                skip(&meta)?;
            }
            n if VARIANT_IGNORED.contains(&n) => skip(&meta)?,
            n => return Err(unknown(&meta, n, "variant")),
        }
        Ok(())
    })?;
    Ok(v)
}

/// An error for a quiddity attribute that applies to an identity in the
/// mode, where serde's attributes are the source of truth.
fn forbidden(span: Span, name: &str, key: &str) -> syn::Error {
    syn::Error::new(
        span,
        format!(
            "`{name}` applies to identity `{key}`, which follows serde's attributes \
             (`serde_attributes`): use serde's attribute instead, or scope this one to other \
             identities, as `{name}(Meaning)`"
        ),
    )
}

/// Applies serde's type attributes to the identities in the mode.
pub fn apply_type(
    attrs: &mut TypeAttrs,
    serde_attrs: &[Attribute],
    ident: &syn::Ident,
    is_struct: bool,
) -> syn::Result<()> {
    let keys = attrs.serde_keys();
    if keys.is_empty() {
        return Ok(());
    }
    let t = parse_type(serde_attrs, ident)?;
    let span = Span::call_site();
    for k in &keys {
        for (name, set) in [
            ("rename_all", attrs.rename_all.get(k).is_some()),
            ("tag", attrs.tag.get(k).is_some()),
            ("content", attrs.content.get(k).is_some()),
            ("untagged", attrs.untagged.get(k).is_some()),
            ("transparent", attrs.transparent.get(k).is_some()),
            (
                "encode_as = serde",
                matches!(attrs.encode_as.get(k), Some((EncodeAs::Serde, _))),
            ),
        ] {
            if set {
                return Err(forbidden(span, name, k));
            }
        }
        if let Some(rule) = t.rename_all {
            attrs.rename_all.set_for(k, rule);
        }
        if let Some(rule) = t.rename_all_fields {
            attrs.rename_all_fields.set_for(k, rule);
        }
        match (&t.tag, is_struct) {
            // serde writes a struct's name under its tag.
            (Some(tag), true) => {
                let name = t
                    .rename
                    .as_ref()
                    .map(LitStr::value)
                    .unwrap_or_else(|| ident.to_string());
                attrs.struct_tag.set_for(k, (tag.clone(), name));
            }
            (Some(tag), false) => attrs.tag.set_for(k, tag.clone()),
            (None, _) => {}
        }
        if let Some(content) = &t.content {
            attrs.content.set_for(k, content.clone());
        }
        if t.untagged {
            attrs.untagged.set_for(k, true);
        }
        if t.transparent {
            attrs.transparent.set_for(k, true);
        }
        if let Some(span) = t.into {
            set_through_serde(attrs, k, span, "into")?;
        }
    }
    Ok(())
}

/// Digests the whole type through its `Serialize` impl, for serde
/// attributes the derive does not follow field by field.
fn set_through_serde(attrs: &mut TypeAttrs, key: &str, span: Span, what: &str) -> syn::Result<()> {
    match attrs.encode_as.get(key) {
        None | Some((EncodeAs::Serde, _)) => {
            attrs.encode_as.set_for(key, (EncodeAs::Serde, span));
            Ok(())
        }
        Some(_) => Err(syn::Error::new(
            span,
            format!(
                "serde's `{what}` makes the type digested through its `Serialize` impl, which \
                 `encode_as` on the type contradicts (identity `{key}`)"
            ),
        )),
    }
}

/// Applies serde's field attributes to the identities in the mode.
pub fn apply_field(
    attrs: &mut FieldAttrs,
    serde_attrs: &[Attribute],
    keys: &[String],
    span: Span,
) -> syn::Result<()> {
    if keys.is_empty() {
        return Ok(());
    }
    let f = parse_field(serde_attrs)?;
    let omit = f.skip_if.as_ref().map(omission).transpose()?;
    for k in keys {
        for (name, set) in [
            ("rename", attrs.rename.get(k).is_some()),
            ("skip", attrs.skip.get(k).is_some()),
            ("omit_none", attrs.omit_none.get(k).is_some()),
            ("omit_empty", attrs.omit_empty.get(k).is_some()),
            ("omit_if", attrs.omit_if.get(k).is_some()),
            ("with", attrs.with.get(k).is_some()),
            ("flatten", attrs.flatten.get(k).is_some()),
            (
                "encode_as = serde",
                matches!(attrs.encode_as.get(k), Some((EncodeAs::Serde, _))),
            ),
        ] {
            if set {
                return Err(forbidden(span, name, k));
            }
        }
        if let Some(rename) = &f.rename {
            attrs.rename.set_for(k, rename.clone());
        }
        if f.skip {
            attrs.skip.set_for(k, true);
        }
        match &omit {
            Some(Omission::None) => attrs.omit_none.set_for(k, true),
            Some(Omission::Empty) => attrs.omit_empty.set_for(k, true),
            Some(Omission::If(path)) => attrs.omit_if.set_for(k, path.clone()),
            None => {}
        }
        if let Some(path) = &f.serialize_with {
            attrs.serialize_with.set_for(k, path.clone());
        }
        if f.flatten {
            attrs.flatten.set_for(k, true);
        }
    }
    Ok(())
}

/// Applies serde's variant attributes to the identities in the mode.
pub fn apply_variant(
    type_attrs: &mut TypeAttrs,
    attrs: &mut VariantAttrs,
    serde_attrs: &[Attribute],
    span: Span,
) -> syn::Result<()> {
    let keys = type_attrs.serde_keys();
    if keys.is_empty() {
        return Ok(());
    }
    let v = parse_variant(serde_attrs)?;
    for k in &keys {
        if attrs.rename.get(k).is_some() {
            return Err(forbidden(span, "rename", k));
        }
        if let Some(rename) = &v.rename {
            attrs.rename.set_for(k, rename.clone());
        }
        if let Some(rule) = v.rename_all {
            attrs.fields_rename_all.set_for(k, rule);
        }
        if let Some((span, what)) = v.through_serde {
            set_through_serde(type_attrs, k, span, what)?;
        }
    }
    Ok(())
}

/// What `skip_serializing_if` maps to.
enum Omission {
    None,
    Empty,
    If(Path),
}

/// The standard predicates `skip_serializing_if` names, by their text
/// without whitespace, bare or with their standard module paths (D21).
fn omission(lit: &LitStr) -> syn::Result<Omission> {
    let text: String = lit.value().chars().filter(|c| !c.is_whitespace()).collect();
    const NONE: &[&str] = &[
        "Option::is_none",
        "std::option::Option::is_none",
        "core::option::Option::is_none",
    ];
    const EMPTY: &[&str] = &[
        "Vec::is_empty",
        "std::vec::Vec::is_empty",
        "alloc::vec::Vec::is_empty",
        "VecDeque::is_empty",
        "std::collections::VecDeque::is_empty",
        "alloc::collections::VecDeque::is_empty",
        "std::collections::vec_deque::VecDeque::is_empty",
        "alloc::collections::vec_deque::VecDeque::is_empty",
        "String::is_empty",
        "std::string::String::is_empty",
        "alloc::string::String::is_empty",
        "str::is_empty",
        "<str>::is_empty",
        "<[_]>::is_empty",
        "HashMap::is_empty",
        "std::collections::HashMap::is_empty",
        "std::collections::hash_map::HashMap::is_empty",
        "HashSet::is_empty",
        "std::collections::HashSet::is_empty",
        "std::collections::hash_set::HashSet::is_empty",
        "BTreeMap::is_empty",
        "std::collections::BTreeMap::is_empty",
        "alloc::collections::BTreeMap::is_empty",
        "std::collections::btree_map::BTreeMap::is_empty",
        "alloc::collections::btree_map::BTreeMap::is_empty",
        "BTreeSet::is_empty",
        "std::collections::BTreeSet::is_empty",
        "alloc::collections::BTreeSet::is_empty",
        "std::collections::btree_set::BTreeSet::is_empty",
        "alloc::collections::btree_set::BTreeSet::is_empty",
        "IndexMap::is_empty",
        "indexmap::IndexMap::is_empty",
        "indexmap::map::IndexMap::is_empty",
        "IndexSet::is_empty",
        "indexmap::IndexSet::is_empty",
        "indexmap::set::IndexSet::is_empty",
        "serde_json::Map::is_empty",
        "serde_json::map::Map::is_empty",
    ];
    if NONE.contains(&text.as_str()) {
        return Ok(Omission::None);
    }
    if EMPTY.contains(&text.as_str()) {
        return Ok(Omission::Empty);
    }
    lit.parse().map(Omission::If).map_err(|_| {
        syn::Error::new_spanned(
            lit,
            "`skip_serializing_if` names a path that `serde_attributes` cannot call; use a \
             function's path",
        )
    })
}
