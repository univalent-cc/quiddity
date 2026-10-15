//! `#[derive(Quiddity)]`: one `impl Quiddity<I>` per identity (spec 6.3–6.5).

use std::collections::BTreeSet;

use proc_macro2::{Span, TokenStream};
use quote::{format_ident, quote, quote_spanned};
use syn::spanned::Spanned;
use syn::visit::Visit;
use syn::{
    Data, DeriveInput, Expr, ExprLit, ExprUnary, Fields, Ident, Lit, LitStr, Path, Type, UnOp,
    Variant,
};

use crate::attr::{EncodeAs, FieldAttrs, PRIMARY, RenameRule, TypeAttrs, VariantAttrs};

/// An identity the type implements.
struct Identity {
    key: String,
    ty: TokenStream,
}

/// A field of a struct or variant, bound by reference as `binding`.
struct Field {
    /// The field's name, for named fields.
    name: Option<Ident>,
    binding: Ident,
    ty: Type,
    attrs: FieldAttrs,
    /// Written as `PhantomData<…>`: never participates (spec 6.3). Other
    /// spellings are caught at run time by `ABSENT_AS_FIELD`.
    phantom: bool,
}

/// How a participating field's value is written.
enum ValueEncoding {
    /// Through the field type's own `Quiddity<J>` impl.
    Own(TokenStream),
    With(Path),
    /// Through a serde `serialize_with` function (`serde_attributes`).
    SerializeWith(Path),
    Display,
    Serde,
    /// The items of a sequence, as a set.
    Set,
    /// Quantized to a multiple of a step.
    Quantize(u64),
    /// Quantized to a number of decimals.
    Decimals(i32),
}

/// The shape a list of fields is written in.
#[derive(Clone, Copy, PartialEq)]
enum Shape {
    Map,
    List,
    /// The value of the one participating field, or null if a newtype's
    /// field does not participate.
    Single,
    /// The value of the one participating field, which must exist.
    Transparent,
}

pub fn derive(input: DeriveInput) -> syn::Result<TokenStream> {
    let mut attrs = TypeAttrs::parse(&input.attrs)?;
    let is_struct = matches!(input.data, Data::Struct(_));
    crate::serde_attrs::apply_type(&mut attrs, &input.attrs, &input.ident, is_struct)?;
    let serde_keys = attrs.serde_keys();
    let keys = attrs.keys();
    let mut identities = vec![Identity {
        key: PRIMARY.to_owned(),
        ty: quote!(::quiddity::Primary),
    }];
    for path in &attrs.identities {
        identities.push(Identity {
            key: crate::attr::identity_key(path),
            ty: quote!(#path),
        });
    }

    let body = match &input.data {
        Data::Struct(data) => Body::Struct(
            fields(&data.fields, &keys, &serde_keys)?,
            data.fields.clone(),
        ),
        Data::Enum(data) => {
            if attrs.transparent.is_set() {
                return Err(syn::Error::new(
                    Span::call_site(),
                    "`transparent` applies to structs only",
                ));
            }
            let mut variants = Vec::new();
            for v in &data.variants {
                let mut vattrs = VariantAttrs::parse(&v.attrs, &keys)?;
                crate::serde_attrs::apply_variant(
                    &mut attrs,
                    &mut vattrs,
                    &v.attrs,
                    v.ident.span(),
                )?;
                variants.push((v, vattrs, fields(&v.fields, &keys, &serde_keys)?));
            }
            Body::Enum(variants)
        }
        Data::Union(u) => {
            return Err(syn::Error::new_spanned(
                u.union_token,
                "unions cannot derive `Quiddity`",
            ));
        }
    };

    let mut impls = TokenStream::new();
    for id in &identities {
        impls.extend(implement(&input, &attrs, &body, id)?);
    }
    Ok(impls)
}

enum Body<'a> {
    Struct(Vec<Field>, Fields),
    Enum(Vec<(&'a Variant, VariantAttrs, Vec<Field>)>),
}

fn fields(fields: &Fields, keys: &[String], serde_keys: &[String]) -> syn::Result<Vec<Field>> {
    let mut out = Vec::new();
    for (i, f) in fields.iter().enumerate() {
        let mut attrs = FieldAttrs::parse(&f.attrs, keys)?;
        crate::serde_attrs::apply_field(&mut attrs, &f.attrs, serde_keys, f.ty.span())?;
        if f.ident.is_none() && attrs.rename.is_set() {
            return Err(syn::Error::new_spanned(
                f,
                "tuple fields have no names to rename",
            ));
        }
        out.push(Field {
            name: f.ident.clone(),
            binding: format_ident!("__quiddity_f{}", i),
            ty: f.ty.clone(),
            phantom: is_phantom(&f.ty),
            attrs,
        });
    }
    Ok(out)
}

fn is_phantom(ty: &Type) -> bool {
    match ty {
        Type::Path(p) => {
            p.qself.is_none()
                && p.path
                    .segments
                    .last()
                    .is_some_and(|s| s.ident == "PhantomData")
        }
        _ => false,
    }
}

fn implement(
    input: &DeriveInput,
    attrs: &TypeAttrs,
    body: &Body,
    id: &Identity,
) -> syn::Result<TokenStream> {
    let name = &input.ident;
    let id_ty = &id.ty;
    let key = id.key.as_str();
    let mut bounds = Bounds::new(input);

    let type_shape = attrs.encode_as.get(key).copied();
    let encode = match (type_shape, body) {
        (Some((EncodeAs::Display, _)), _) => quote!(__e.display(self);),
        (Some((EncodeAs::Serde, _)), _) => quote!(::quiddity::__encode_serde!(self, __e);),
        (_, Body::Struct(fields, form)) => {
            let shape = struct_shape(attrs, key, form, type_shape)?;
            let rename_all = attrs.rename_all.get(key).copied();
            let extra = struct_tag(attrs, key, shape)?;
            let write = write_fields(fields, shape, key, rename_all, extra, &mut bounds)?;
            let pattern = pattern(quote!(Self), fields, form);
            quote! {
                let #pattern = self;
                #write
            }
        }
        (_, Body::Enum(variants)) => {
            let int = enum_int(type_shape)?;
            let rename_all = attrs.rename_all.get(key).copied();
            let fields_rule = attrs.rename_all_fields.get(key).copied();
            let style = enum_style(attrs, key, int)?;
            write_enum(
                variants,
                int,
                &style,
                key,
                rename_all,
                fields_rule,
                &mut bounds,
            )?
        }
    };
    if matches!(body, Body::Struct(..))
        && (attrs.tag.is_set() || attrs.content.is_set() || attrs.untagged.is_set())
    {
        return Err(syn::Error::new(
            Span::call_site(),
            "`tag`, `content` and `untagged` apply to enums only",
        ));
    }
    let entries = flatten_entries(attrs, body, key, &mut bounds)?;
    let describe = describe(attrs, body, key, id_ty)?;

    let domain = match attrs.domain.get(key) {
        Some(label) => {
            quote!(const DOMAIN: ::core::option::Option<&'static str> = ::core::option::Option::Some(#label);)
        }
        None => quote!(),
    };
    let (impl_generics, ty_generics, _) = input.generics.split_for_impl();
    let where_clause = bounds.where_clause();
    let flatten = match entries {
        Some(entries) => quote! {
            ::quiddity::__if_alloc! {
                const __FLATTENABLE: bool = true;

                #[allow(unused_variables, clippy::needless_borrow)]
                fn __encode_entries(&self, __m: &mut ::quiddity::__private::Flattened<'_>) {
                    #entries
                }
            }
        },
        None => quote!(),
    };
    Ok(quote! {

        #[automatically_derived]
        impl #impl_generics ::quiddity::Quiddity<#id_ty> for #name #ty_generics #where_clause {
            #domain

            #flatten

            #[allow(unused_variables, clippy::needless_borrow)]
            fn encode(&self, __e: &mut ::quiddity::Encoder) {
                #encode
            }

            ::quiddity::__if_identity! {
                #[allow(unused_variables, unreachable_code, clippy::needless_question_mark)]
                fn __describe(
                    __cx: &mut ::quiddity::identity::export::Exporter,
                ) -> ::core::result::Result<::quiddity::identity::Kind, ::quiddity::identity::ExportError> {
                    __cx.define::<Self, #id_ty>(|__cx| ::core::result::Result::Ok(#describe))
                }
            }
        }
    })
}

/// The body of the type's `__encode_entries` under identity `key` (D21):
/// statements writing its members into `__m`. `None` for types that have
/// no members to flatten: structs not encoded as a map, enums encoded with
/// integer tags, and types encoded through `Display` or `Serialize`.
fn flatten_entries(
    attrs: &TypeAttrs,
    body: &Body,
    key: &str,
    bounds: &mut Bounds,
) -> syn::Result<Option<TokenStream>> {
    let type_shape = attrs.encode_as.get(key).copied();
    if matches!(type_shape, Some((EncodeAs::Display | EncodeAs::Serde, _))) {
        return Ok(None);
    }
    match body {
        Body::Struct(fields, form) => {
            if struct_shape(attrs, key, form, type_shape)? != Shape::Map {
                return Ok(None);
            }
            let rename_all = attrs.rename_all.get(key).copied();
            let extra = struct_tag(attrs, key, Shape::Map)?;
            let (writes, flattened) = map_writes(fields, key, rename_all, extra, bounds)?;
            let pattern = pattern(quote!(Self), fields, form);
            Ok(Some(quote! {
                let #pattern = self;
                #writes
                #flattened
            }))
        }
        Body::Enum(variants) => {
            if enum_int(type_shape)? {
                return Ok(None);
            }
            let rename_all = attrs.rename_all.get(key).copied();
            let style = enum_style(attrs, key, false)?;
            let mut arms = TokenStream::new();
            for (v, vattrs, fields) in variants {
                let ident = &v.ident;
                let name = variant_name(v, vattrs, key, rename_all);
                let fr = fields_rule(attrs, vattrs, key);
                let lit = LitStr::new(&name, Span::call_site());
                let form = &v.fields;
                let pattern = pattern(quote!(Self::#ident), fields, form);
                let unit = matches!(form, Fields::Unit);
                let named = matches!(form, Fields::Named(_));
                let label = quote!(__e.str(#lit););
                let write = match &style {
                    Style::External if unit => {
                        let what = format!("the unit variant `{name}`");
                        quote!(__m.cannot_flatten(#what);)
                    }
                    Style::External => {
                        let data =
                            write_fields(fields, variant_shape(form), key, fr, None, bounds)?;
                        quote!(__m.field_with(#lit, |__e| { #data });)
                    }
                    Style::Internal(tag) if unit => quote!(__m.field_with(#tag, |__e| { #label });),
                    Style::Internal(tag) => {
                        let (writes, flattened) =
                            map_writes(fields, key, fr, Some((tag.value(), label)), bounds)?;
                        quote!(#writes #flattened)
                    }
                    Style::Adjacent(tag, _) if unit => {
                        quote!(__m.field_with(#tag, |__e| { #label });)
                    }
                    Style::Adjacent(tag, content) => {
                        let data =
                            write_fields(fields, variant_shape(form), key, fr, None, bounds)?;
                        quote! {
                            __m.field_with(#tag, |__e| { #label });
                            __m.field_with(#content, |__e| { #data });
                        }
                    }
                    Style::Untagged if unit => quote!(),
                    Style::Untagged if named => {
                        let (writes, flattened) = map_writes(fields, key, fr, None, bounds)?;
                        quote!(#writes #flattened)
                    }
                    Style::Untagged => {
                        let what = format!("the variant `{name}`, which is not a struct variant");
                        quote!(__m.cannot_flatten(#what);)
                    }
                };
                arms.extend(quote!(#pattern => { #write }));
            }
            Ok(Some(quote! {
                match self {
                    #arms
                }
            }))
        }
    }
}

/// The case convention for a struct variant's fields under `key`: the
/// variant's serde `rename_all`, else the enum's `rename_all_fields`.
fn fields_rule(attrs: &TypeAttrs, vattrs: &VariantAttrs, key: &str) -> Option<RenameRule> {
    vattrs
        .fields_rename_all
        .get(key)
        .copied()
        .or(attrs.rename_all_fields.get(key).copied())
}

/// A struct's serde `tag` under `key`, as the extra member it writes: the
/// tag's name, and the struct's name as its value.
fn struct_tag(
    attrs: &TypeAttrs,
    key: &str,
    shape: Shape,
) -> syn::Result<Option<(String, TokenStream)>> {
    let Some((tag, name)) = attrs.struct_tag.get(key) else {
        return Ok(None);
    };
    if shape != Shape::Map {
        return Err(syn::Error::new_spanned(
            tag,
            "serde's `tag` on a struct applies to structs with named fields encoded as a map",
        ));
    }
    Ok(Some((tag.value(), quote!(__e.str(#name);))))
}

/// The shape of a struct's fields under one identity.
fn struct_shape(
    attrs: &TypeAttrs,
    key: &str,
    form: &Fields,
    type_shape: Option<(EncodeAs, Span)>,
) -> syn::Result<Shape> {
    if attrs.transparent.get(key).copied().unwrap_or(false) {
        return Ok(Shape::Transparent);
    }
    match (form, type_shape) {
        (Fields::Named(_), None | Some((EncodeAs::Map, _))) => Ok(Shape::Map),
        (Fields::Named(_), Some((EncodeAs::List, _))) => Ok(Shape::List),
        (Fields::Unnamed(f), None) if f.unnamed.len() == 1 => Ok(Shape::Single),
        (Fields::Unnamed(_), None) => Ok(Shape::List),
        (Fields::Unit, None) => Ok(Shape::List), // no fields: written as null below
        (Fields::Named(_), Some((_, span))) => Err(syn::Error::new(
            span,
            "on a struct with named fields, `encode_as` must be `map`, `list`, `display` or `serde`",
        )),
        (_, Some((_, span))) => Err(syn::Error::new(
            span,
            "on a tuple or unit struct, `encode_as` must be `display` or `serde`",
        )),
    }
}

/// The pattern binding each field by reference.
fn pattern(path: TokenStream, fields: &[Field], form: &Fields) -> TokenStream {
    let bindings = fields.iter().map(|f| &f.binding);
    match form {
        Fields::Named(_) => {
            let names = fields.iter().map(|f| f.name.as_ref().unwrap());
            quote!(#path { #(#names: #bindings),* })
        }
        Fields::Unnamed(_) => quote!(#path ( #(#bindings),* )),
        Fields::Unit => quote!(#path),
    }
}

impl Field {
    fn participates(&self, key: &str) -> bool {
        !self.phantom && !self.attrs.skip.get(key).copied().unwrap_or(false)
    }

    /// Whether the field's members are written into the containing map
    /// under `key` (D21). Checks that nothing else is said about the
    /// field's key or value, which a flattened field does not have.
    fn flattens(&self, key: &str) -> syn::Result<bool> {
        if !self.attrs.flatten.get(key).copied().unwrap_or(false) || !self.participates(key) {
            return Ok(false);
        }
        let a = &self.attrs;
        let conflicts = [
            ("rename", a.rename.get(key).is_some()),
            ("omit_none", a.omit_none.get(key).is_some()),
            ("omit_empty", a.omit_empty.get(key).is_some()),
            ("omit_if", a.omit_if.get(key).is_some()),
            ("with", a.with.get(key).is_some()),
            ("encode_as", a.encode_as.get(key).is_some()),
            ("quantize", a.quantize.get(key).is_some()),
            ("decimals", a.decimals.get(key).is_some()),
        ];
        if let Some((name, _)) = conflicts.iter().find(|(_, set)| *set) {
            return Err(syn::Error::new_spanned(
                &self.ty,
                format!(
                    "`flatten` cannot be combined with `{name}`: a flattened field has no key or \
                     value of its own (identity `{key}`)"
                ),
            ));
        }
        if self.name.is_none() {
            return Err(syn::Error::new_spanned(
                &self.ty,
                "`flatten` applies to named fields, not tuple fields",
            ));
        }
        Ok(true)
    }

    /// The identity under which a flattened field's members are written.
    fn flatten_identity(&self, key: &str) -> TokenStream {
        match self.attrs.identity.get(key) {
            Some(j) => quote!(#j),
            None => quote!(::quiddity::Primary),
        }
    }

    fn encoding(&self, key: &str) -> syn::Result<ValueEncoding> {
        let a = &self.attrs;
        let (with, shape, identity) = (a.with.get(key), a.encode_as.get(key), a.identity.get(key));
        let (quantize, decimals) = (a.quantize.get(key), a.decimals.get(key));
        let serialize_with = a.serialize_with.get(key);
        if [
            serialize_with.is_some(),
            with.is_some(),
            shape.is_some(),
            identity.is_some(),
            quantize.is_some(),
            decimals.is_some(),
        ]
        .iter()
        .filter(|b| **b)
        .count()
            > 1
        {
            return Err(syn::Error::new_spanned(
                &self.ty,
                format!(
                    "`with`, `encode_as`, `identity`, `quantize`, `decimals` and serde's \
                     `serialize_with` cannot be \
                     combined (identity `{key}`)"
                ),
            ));
        }
        if let Some(f) = serialize_with {
            return Ok(ValueEncoding::SerializeWith(f.clone()));
        }
        if let Some(q) = quantize {
            return Ok(ValueEncoding::Quantize(*q));
        }
        if let Some(d) = decimals {
            return Ok(ValueEncoding::Decimals(*d));
        }
        Ok(match (with, shape, identity) {
            (Some(path), _, _) => ValueEncoding::With(path.clone()),
            (_, Some((EncodeAs::Display, _)), _) => ValueEncoding::Display,
            (_, Some((EncodeAs::Set, _)), _) => ValueEncoding::Set,
            (_, Some(_), _) => ValueEncoding::Serde,
            (_, _, Some(j)) => ValueEncoding::Own(quote!(#j)),
            _ => ValueEncoding::Own(quote!(::quiddity::Primary)),
        })
    }

    /// Statements writing the field's value through `__e`.
    fn write_value(&self, key: &str, bounds: &mut Bounds) -> syn::Result<TokenStream> {
        let (b, ty) = (&self.binding, &self.ty);
        Ok(match self.encoding(key)? {
            ValueEncoding::Own(j) => {
                bounds.add(ty, &j);
                quote!(<#ty as ::quiddity::Quiddity<#j>>::encode(#b, __e);)
            }
            ValueEncoding::With(path) => quote!(#path(#b, __e);),
            ValueEncoding::SerializeWith(path) => {
                quote!(::quiddity::__encode_serialize_with!(__e, |__s| #path(#b, __s));)
            }
            ValueEncoding::Display => quote!(__e.display(#b);),
            ValueEncoding::Serde => quote!(::quiddity::__encode_serde!(#b, __e);),
            ValueEncoding::Quantize(q) => quote_spanned!(ty.span()=>
                ::quiddity::__private::Quantize::encode_quantized(#b, #q, __e);
            ),
            ValueEncoding::Decimals(d) => quote_spanned!(ty.span()=>
                ::quiddity::__private::Decimals::encode_decimals(#b, #d, __e);
            ),
            ValueEncoding::Set => {
                // The items are written under their primary identity.
                bounds.add(ty, &quote!(::quiddity::Primary));
                quote!(::quiddity::__encode_set!(#b, __e);)
            }
        })
    }

    /// The condition under which the field is written at all: not absent
    /// (`PhantomData` under another name), and not omitted.
    fn condition(&self, key: &str, shape: Shape) -> syn::Result<TokenStream> {
        let (b, ty) = (&self.binding, &self.ty);
        let mut cond = match self.encoding(key)? {
            ValueEncoding::Own(j) => quote!(!<#ty as ::quiddity::Quiddity<#j>>::ABSENT_AS_FIELD),
            _ => quote!(true),
        };
        let none = self.attrs.omit_none.get(key).copied().unwrap_or(false);
        let empty = self.attrs.omit_empty.get(key).copied().unwrap_or(false);
        let omit_if = self.attrs.omit_if.get(key);
        if (none || empty || omit_if.is_some()) && shape != Shape::Map {
            return Err(syn::Error::new_spanned(
                &self.ty,
                "`omit_none`, `omit_empty` and `omit_if` apply only to fields encoded as a map",
            ));
        }
        if none {
            cond.extend(
                quote_spanned!(ty.span()=> && !::quiddity::__private::Nullable::is_null(#b)),
            );
        }
        if empty {
            cond.extend(
                quote_spanned!(ty.span()=> && !::quiddity::__private::IsEmpty::is_empty(#b)),
            );
        }
        if let Some(path) = omit_if {
            cond.extend(quote!(&& !#path(#b)));
        }
        Ok(cond)
    }
}

/// The participating fields of a struct with named fields, with their keys
/// under the identity `key`, in canonical order.
fn map_entries<'f>(
    fields: &'f [Field],
    key: &str,
    rename_all: Option<RenameRule>,
) -> syn::Result<Vec<(String, &'f Field)>> {
    let mut entries = Vec::new();
    for f in fields.iter().filter(|f| f.participates(key)) {
        if f.flattens(key)? {
            continue;
        }
        let name = match (f.attrs.rename.get(key), rename_all) {
            (Some(lit), _) => lit.value(),
            (None, Some(rule)) => rule.apply_to_field(&unraw(f.name.as_ref().unwrap())),
            (None, None) => unraw(f.name.as_ref().unwrap()),
        };
        entries.push((name, f));
    }
    entries.sort_by(|a, b| cmp_key_encodings(&a.0, &b.0));
    if let Some(w) = entries.windows(2).find(|w| w[0].0 == w[1].0) {
        return Err(syn::Error::new_spanned(
            &w[1].1.ty,
            format!(
                "two fields have the key {:?} under identity `{key}`",
                w[1].0
            ),
        ));
    }
    Ok(entries)
}

/// Statements writing `fields` in `shape` under the identity `key`. With
/// `extra`, a further map entry (an internally tagged enum's tag) is
/// written among the fields, in canonical order.
fn write_fields(
    fields: &[Field],
    shape: Shape,
    key: &str,
    rename_all: Option<RenameRule>,
    extra: Option<(String, TokenStream)>,
    bounds: &mut Bounds,
) -> syn::Result<TokenStream> {
    let participating: Vec<&Field> = fields.iter().filter(|f| f.participates(key)).collect();
    // Check omission attributes even on fields that do not participate.
    for f in fields.iter().filter(|f| f.participates(key)) {
        if f.flattens(key)? {
            if shape != Shape::Map {
                return Err(syn::Error::new_spanned(
                    &f.ty,
                    "`flatten` applies only to fields encoded as a map: not with \
                     `encode_as = list` or `transparent`",
                ));
            }
            continue;
        }
        f.condition(key, shape)?;
    }
    match shape {
        Shape::Single if fields.is_empty() => Ok(quote!(__e.null();)),
        Shape::Single => {
            if participating.len() == 1 {
                participating[0].write_value(key, bounds)
            } else if fields.len() == 1 {
                // A newtype whose one field is skipped.
                Ok(quote!(__e.null();))
            } else {
                Err(syn::Error::new(
                    Span::call_site(),
                    format!(
                        "`transparent` needs exactly one participating field, but identity \
                         `{key}` has {}",
                        participating.len()
                    ),
                ))
            }
        }
        Shape::Transparent => {
            if participating.len() != 1 {
                return Err(syn::Error::new(
                    Span::call_site(),
                    format!(
                        "`transparent` needs exactly one participating field, but identity \
                         `{key}` has {}",
                        participating.len()
                    ),
                ));
            }
            let field = participating[0];
            let value = field.write_value(key, bounds)?;
            // A generic field can turn out to be `PhantomData`, which does
            // not participate: check once the type is known.
            let check = match field.encoding(key)? {
                ValueEncoding::Own(j) => {
                    let ty = &field.ty;
                    quote! {
                        const {
                            ::core::assert!(
                                !<#ty as ::quiddity::Quiddity<#j>>::ABSENT_AS_FIELD,
                                "`transparent` needs exactly one participating field, \
                                 but this one is `PhantomData`",
                            )
                        };
                    }
                }
                _ => quote!(),
            };
            Ok(quote!(#check #value))
        }
        Shape::List if fields.is_empty() => Ok(quote!(__e.null();)),
        Shape::List => {
            let mut items = TokenStream::new();
            for f in participating {
                let cond = f.condition(key, shape)?;
                let value = f.write_value(key, bounds)?;
                items.extend(quote!(if #cond { __l.item_with(|__e| { #value }); }));
            }
            Ok(quote! {
                let mut __l = __e.list();
                #items
                __l.finish();
            })
        }
        Shape::Map => {
            let (writes, flattened) = map_writes(fields, key, rename_all, extra, bounds)?;
            if flattened.is_empty() {
                Ok(quote! {
                    let mut __m = __e.fields();
                    #writes
                    __m.finish();
                })
            } else {
                // The flattened members' keys are known only at run time:
                // a buffered map, sorted when finished (D21).
                Ok(quote! {
                    let mut __flat = ::quiddity::__flattened!(__e, ::core::any::type_name::<Self>());
                    {
                        let __m = &mut __flat;
                        #writes
                        #flattened
                    }
                    __flat.finish();
                })
            }
        }
    }
}

/// Statements writing the fields of a map through `__m`, a fields or
/// flattened writer: the named fields (and `extra`) in canonical order, and
/// separately the flattened fields' members.
fn map_writes(
    fields: &[Field],
    key: &str,
    rename_all: Option<RenameRule>,
    extra: Option<(String, TokenStream)>,
    bounds: &mut Bounds,
) -> syn::Result<(TokenStream, TokenStream)> {
    let entries = map_entries(fields, key, rename_all)?;
    let mut writes = Vec::new();
    for (name, f) in entries {
        let cond = f.condition(key, Shape::Map)?;
        let value = f.write_value(key, bounds)?;
        let lit = LitStr::new(&name, Span::call_site());
        writes.push((
            name,
            quote!(if #cond { __m.field_with(#lit, |__e| { #value }); }),
        ));
    }
    if let Some((tag, value)) = extra {
        if writes.iter().any(|(n, _)| *n == tag) {
            return Err(syn::Error::new(
                Span::call_site(),
                format!("a field has the key {tag:?}, which is also the tag"),
            ));
        }
        let lit = LitStr::new(&tag, Span::call_site());
        writes.push((tag, quote!(__m.field_with(#lit, |__e| { #value });)));
    }
    writes.sort_by(|a, b| cmp_key_encodings(&a.0, &b.0));
    let writes: TokenStream = writes.into_iter().map(|(_, w)| w).collect();
    let mut flattened = TokenStream::new();
    for f in fields.iter().filter(|f| f.participates(key)) {
        if f.flattens(key)? {
            let (b, ty) = (&f.binding, &f.ty);
            let j = f.flatten_identity(key);
            bounds.add(ty, &j);
            flattened.extend(quote_spanned!(ty.span()=>
                const {
                    ::core::assert!(
                        <#ty as ::quiddity::Quiddity<#j>>::__FLATTENABLE,
                        "`flatten` applies to structs with named fields, enums and maps, and \
                         `Option`s of them",
                    )
                };
                <#ty as ::quiddity::Quiddity<#j>>::__encode_entries(#b, __m);
            ));
        }
    }
    Ok((writes, flattened))
}

fn unraw(ident: &Ident) -> String {
    let s = ident.to_string();
    s.strip_prefix("r#").map(str::to_owned).unwrap_or(s)
}

/// The canonical order of string keys (spec 2.5): by their encodings.
fn cmp_key_encodings(a: &str, b: &str) -> std::cmp::Ordering {
    fn encoding(s: &str) -> impl Iterator<Item = u8> + '_ {
        s.bytes()
            .chain((s.len() as u64).to_be_bytes())
            .chain([0x20])
    }
    encoding(a).cmp(encoding(b))
}

/// How an enum's variants are tagged (spec 6.4).
enum Style {
    External,
    Internal(LitStr),
    Adjacent(LitStr, LitStr),
    Untagged,
}

fn enum_int(type_shape: Option<(EncodeAs, Span)>) -> syn::Result<bool> {
    match type_shape {
        None | Some((EncodeAs::String, _)) => Ok(false),
        Some((EncodeAs::Int, _)) => Ok(true),
        Some((_, span)) => Err(syn::Error::new(
            span,
            "on an enum, `encode_as` must be `string`, `int`, `display` or `serde`",
        )),
    }
}

fn enum_style(attrs: &TypeAttrs, key: &str, int: bool) -> syn::Result<Style> {
    let untagged = attrs.untagged.get(key).copied().unwrap_or(false);
    let tag = attrs.tag.get(key).cloned();
    let content = attrs.content.get(key).cloned();
    match (untagged, tag, content) {
        (true, None, None) if int => Err(syn::Error::new(
            Span::call_site(),
            "an untagged enum has no tag for `encode_as = int` to apply to",
        )),
        (true, None, None) => Ok(Style::Untagged),
        (true, ..) => Err(syn::Error::new(
            Span::call_site(),
            "`untagged` cannot be combined with `tag` or `content`",
        )),
        (false, None, None) => Ok(Style::External),
        (false, None, Some(c)) => Err(syn::Error::new_spanned(c, "`content` needs `tag`")),
        (false, Some(t), None) => Ok(Style::Internal(t)),
        (false, Some(t), Some(c)) if t.value() == c.value() => Err(syn::Error::new_spanned(
            c,
            "`content` must differ from `tag`",
        )),
        (false, Some(t), Some(c)) => Ok(Style::Adjacent(t, c)),
    }
}

/// A variant's name under identity `key`: its `rename`, or its identifier
/// with `rename_all` applied.
fn variant_name(
    v: &Variant,
    attrs: &VariantAttrs,
    key: &str,
    rename_all: Option<RenameRule>,
) -> String {
    match (attrs.rename.get(key), rename_all) {
        (Some(lit), _) => lit.value(),
        (None, Some(rule)) => rule.apply_to_variant(&unraw(&v.ident)),
        (None, None) => unraw(&v.ident),
    }
}

/// The shape a variant's fields are written in.
fn variant_shape(form: &Fields) -> Shape {
    match form {
        Fields::Named(_) => Shape::Map,
        Fields::Unnamed(f) if f.unnamed.len() == 1 => Shape::Single,
        _ => Shape::List,
    }
}

/// Each variant's discriminant, as an `i128` expression.
fn discriminants(variants: &[(&Variant, VariantAttrs, Vec<Field>)]) -> Vec<TokenStream> {
    let unit_only = variants
        .iter()
        .all(|(v, ..)| matches!(v.fields, Fields::Unit));
    let mut previous: Option<TokenStream> = None;
    let mut out = Vec::new();
    for (v, ..) in variants {
        let ident = &v.ident;
        let d = if unit_only {
            quote!((Self::#ident as i128))
        } else {
            match (&v.discriminant, &previous) {
                (Some((_, expr)), _) => discriminant(expr),
                (None, Some(prev)) => quote!((#prev + 1i128)),
                (None, None) => quote!(0i128),
            }
        };
        previous = Some(d.clone());
        out.push(d);
    }
    out
}

/// Rejects untagged and internally tagged enums whose variants their
/// styles cannot represent or tell apart, as far as the syntax shows; the
/// full check is made by `construct_identity` (spec 6.8).
fn check_style(
    variants: &[(&Variant, VariantAttrs, Vec<Field>)],
    style: &Style,
) -> syn::Result<()> {
    match style {
        Style::Internal(_) => {
            for (v, ..) in variants {
                if matches!(v.fields, Fields::Unnamed(_)) {
                    return Err(syn::Error::new_spanned(
                        v,
                        "an internally tagged enum's variants must be unit or struct variants",
                    ));
                }
            }
            Ok(())
        }
        Style::Untagged => {
            let mut seen: Vec<(String, &Variant)> = Vec::new();
            for (v, ..) in variants {
                let class = match &v.fields {
                    Fields::Unit => "a unit variant (null)".to_owned(),
                    // Struct variants are told apart by their keys (spec
                    // 5.7); identical key sets are rejected in `write_enum`,
                    // the rest when the identity is exported.
                    Fields::Named(_) => continue,
                    Fields::Unnamed(f) if f.unnamed.len() == 1 => {
                        let ty = &f.unnamed[0].ty;
                        format!("a newtype variant of `{}`", quote!(#ty))
                    }
                    Fields::Unnamed(f) => format!("a tuple variant of {} fields", f.unnamed.len()),
                };
                if let Some((_, first)) = seen.iter().find(|(c, _)| *c == class) {
                    return Err(syn::Error::new_spanned(
                        v,
                        format!(
                            "an untagged enum cannot tell this variant from `{}`: both are {class}",
                            first.ident
                        ),
                    ));
                }
                seen.push((class, v));
            }
            Ok(())
        }
        Style::External | Style::Adjacent(..) => Ok(()),
    }
}

/// Writes an enum. `rename_all` applies to variant names, not to the fields
/// of struct variants, as in serde.
fn write_enum(
    variants: &[(&Variant, VariantAttrs, Vec<Field>)],
    int: bool,
    style: &Style,
    key: &str,
    rename_all: Option<RenameRule>,
    rename_all_fields: Option<RenameRule>,
    bounds: &mut Bounds,
) -> syn::Result<TokenStream> {
    check_style(variants, style)?;
    let fields_rule =
        |v: &VariantAttrs| v.fields_rename_all.get(key).copied().or(rename_all_fields);
    if let Style::Untagged = style {
        let mut key_sets: Vec<(BTreeSet<String>, &Variant)> = Vec::new();
        for (v, vattrs, fields) in variants {
            if !matches!(v.fields, Fields::Named(_)) {
                continue;
            }
            let keys: BTreeSet<String> = map_entries(fields, key, fields_rule(vattrs))?
                .into_iter()
                .map(|(k, _)| k)
                .collect();
            if let Some((_, first)) = key_sets.iter().find(|(k, _)| *k == keys) {
                return Err(syn::Error::new_spanned(
                    v,
                    format!(
                        "an untagged enum cannot tell this variant from `{}`: \
                         both are objects with the keys {keys:?}",
                        first.ident
                    ),
                ));
            }
            key_sets.push((keys, v));
        }
    }
    let mut names = BTreeSet::new();
    let mut arms = TokenStream::new();
    for ((v, attrs, fields), disc) in variants.iter().zip(discriminants(variants)) {
        let ident = &v.ident;
        let name = variant_name(v, attrs, key, rename_all);
        let fr = fields_rule(attrs);
        if !names.insert(name.clone()) {
            return Err(syn::Error::new_spanned(
                v,
                format!("two variants have the name {name:?} under identity `{key}`"),
            ));
        }
        let form = &v.fields;
        let pattern = pattern(quote!(Self::#ident), fields, form);
        let shape = variant_shape(form);
        let lit = LitStr::new(&name, Span::call_site());
        let label = if int {
            quote!(__e.int(#disc);)
        } else {
            quote!(__e.str(#lit);)
        };
        let unit = matches!(form, Fields::Unit);
        let write = match style {
            Style::External if unit => label,
            Style::External => {
                let data = write_fields(fields, shape, key, fr, None, bounds)?;
                if int {
                    quote! {
                        let mut __l = __e.list();
                        __l.item_with(|__e| { #label });
                        __l.item_with(|__e| { #data });
                        __l.finish();
                    }
                } else {
                    quote! {
                        let mut __m = __e.fields();
                        __m.field_with(#lit, |__e| { #data });
                        __m.finish();
                    }
                }
            }
            Style::Internal(tag) if unit => quote! {
                let mut __m = __e.fields();
                __m.field_with(#tag, |__e| { #label });
                __m.finish();
            },
            Style::Internal(tag) => {
                write_fields(fields, shape, key, fr, Some((tag.value(), label)), bounds)?
            }
            Style::Adjacent(tag, _) if unit => quote! {
                let mut __m = __e.fields();
                __m.field_with(#tag, |__e| { #label });
                __m.finish();
            },
            Style::Adjacent(tag, content) => {
                let data = write_fields(fields, shape, key, fr, None, bounds)?;
                let mut entries = vec![
                    (tag.value(), quote!(__m.field_with(#tag, |__e| { #label });)),
                    (
                        content.value(),
                        quote!(__m.field_with(#content, |__e| { #data });),
                    ),
                ];
                entries.sort_by(|a, b| cmp_key_encodings(&a.0, &b.0));
                let writes = entries.into_iter().map(|(_, w)| w);
                quote! {
                    let mut __m = __e.fields();
                    #(#writes)*
                    __m.finish();
                }
            }
            Style::Untagged if unit => quote!(__e.null();),
            Style::Untagged => write_fields(fields, shape, key, fr, None, bounds)?,
        };
        arms.extend(quote!(#pattern => { #write }));
    }
    Ok(quote! {
        match self {
            #arms
        }
    })
}

// --- Export (spec 6.8) ---------------------------------------------------------

/// An expression giving the kind of a type under identity `key`, inside
/// `Exporter::define`'s closure, where `__cx` is the exporter.
fn describe(
    attrs: &TypeAttrs,
    body: &Body,
    key: &str,
    _id_ty: &TokenStream,
) -> syn::Result<TokenStream> {
    let unsupported = |what: &str| {
        quote!(return ::core::result::Result::Err(
            __cx.unsupported_attribute(::core::any::type_name::<Self>(), #what)
        ))
    };
    let type_shape = attrs.encode_as.get(key).copied();
    match (type_shape, body) {
        (Some((EncodeAs::Display, _)), _) => Ok(unsupported("encode_as = display")),
        (Some((EncodeAs::Serde, _)), _) => Ok(unsupported("encode_as = serde")),
        (_, Body::Struct(fields, form)) => {
            let shape = struct_shape(attrs, key, form, type_shape)?;
            let rename_all = attrs.rename_all.get(key).copied();
            let tag = attrs.struct_tag.get(key).map(|(t, _)| t.value());
            describe_fields(fields, form, shape, key, rename_all, tag)
        }
        (_, Body::Enum(variants)) => {
            let int = enum_int(type_shape)?;
            let rename_all = attrs.rename_all.get(key).copied();
            let style = enum_style(attrs, key, int)?;
            let mut alternatives = Vec::new();
            for ((v, vattrs, fields), disc) in variants.iter().zip(discriminants(variants)) {
                let name = variant_name(v, vattrs, key, rename_all);
                let kind = match &v.fields {
                    Fields::Unit => None,
                    form => Some(within(
                        &name,
                        describe_fields(
                            fields,
                            form,
                            variant_shape(form),
                            key,
                            fields_rule(attrs, vattrs, key),
                            None,
                        )?,
                    )),
                };
                alternatives.push((name, kind, disc));
            }
            if let Style::Untagged = style {
                let kinds = alternatives.into_iter().map(|(_, kind, _)| match kind {
                    Some(k) => k,
                    None => quote!(::quiddity::identity::Kind::Null),
                });
                return Ok(quote!(::quiddity::identity::Kind::Union(
                    ::quiddity::__private::vec![#(#kinds),*]
                )));
            }
            let variants = alternatives.into_iter().map(|(name, kind, disc)| {
                let kind = match kind {
                    Some(k) => quote!(::core::option::Option::Some(#k)),
                    None => quote!(::core::option::Option::None),
                };
                let disc = if int {
                    quote!(::core::option::Option::Some(#disc))
                } else {
                    quote!(::core::option::Option::None)
                };
                quote!(::quiddity::identity::export::variant(#name, #kind, #disc))
            });
            let none = quote!(::core::option::Option::None);
            let (style, tag, content) = match &style {
                Style::External => (none.clone(), none.clone(), none),
                Style::Internal(t) => (
                    quote!(::core::option::Option::Some(
                        ::quiddity::identity::Style::Internal
                    )),
                    quote!(::core::option::Option::Some(#t)),
                    none,
                ),
                Style::Adjacent(t, c) => (
                    quote!(::core::option::Option::Some(
                        ::quiddity::identity::Style::Adjacent
                    )),
                    quote!(::core::option::Option::Some(#t)),
                    quote!(::core::option::Option::Some(#c)),
                ),
                Style::Untagged => unreachable!(),
            };
            Ok(quote!(::quiddity::identity::export::tagged(
                ::quiddity::__private::vec![#(#variants),*], #style, #tag, #content, #int
            )))
        }
    }
}

/// An expression giving the kind of a struct's or variant's fields.
fn describe_fields(
    fields: &[Field],
    form: &Fields,
    shape: Shape,
    key: &str,
    rename_all: Option<RenameRule>,
    tag: Option<String>,
) -> syn::Result<TokenStream> {
    let participating: Vec<&Field> = fields.iter().filter(|f| f.participates(key)).collect();
    let null = quote!(::quiddity::identity::Kind::Null);
    if !matches!(form, Fields::Named(_)) && participating.iter().any(|f| f.is_quantized(key)) {
        return Ok(quote!(
            return ::core::result::Result::Err(__cx.unsupported_attribute(
                ::core::any::type_name::<Self>(),
                "quantize or decimals on a field without a name"
            ))
        ));
    }
    match shape {
        Shape::Single | Shape::Transparent => match participating.as_slice() {
            [f] => f.describe_value(key),
            _ => Ok(null),
        },
        Shape::List if matches!(form, Fields::Named(_)) => {
            let entries = map_entries(fields, key, rename_all)?;
            let mut items = Vec::new();
            // A list-encoded struct lists its fields in declaration order.
            for f in &participating {
                let name = entries
                    .iter()
                    .find(|(_, e)| std::ptr::eq(*e, *f))
                    .unwrap()
                    .0
                    .clone();
                let kind = within(&name, f.describe_value(key)?);
                items.push(
                    {
                        let (q, d) = f.quantization(key);
                        quote!((#name, ::quiddity::identity::export::field_quantized(#kind, false, false, #q, #d)))
                    },
                );
            }
            Ok(quote!(::quiddity::identity::export::object(
                ::quiddity::__private::vec![#(#items),*],
                true
            )))
        }
        Shape::List if fields.is_empty() => Ok(null),
        Shape::List => {
            let kinds = participating
                .iter()
                .enumerate()
                .map(|(i, f)| Ok(within(&i.to_string(), f.describe_value(key)?)))
                .collect::<syn::Result<Vec<_>>>()?;
            Ok(quote!(::quiddity::identity::Kind::Tuple(
                ::quiddity::__private::vec![#(#kinds),*]
            )))
        }
        Shape::Map => {
            let mut items = Vec::new();
            if let Some(tag) = tag {
                // A struct's serde tag: its name, a string.
                items.push(quote!((#tag, ::quiddity::identity::export::field(
                    ::quiddity::identity::Kind::String, false, false
                ))));
            }
            let mut flattened = Vec::new();
            for f in participating
                .iter()
                .filter(|f| f.flattens(key).unwrap_or(false))
            {
                let name = unraw(f.name.as_ref().unwrap());
                let kind = within(&name, f.describe_value(key)?);
                flattened.push(quote!((#name, #kind)));
            }
            for (name, f) in map_entries(fields, key, rename_all)? {
                let kind = within(&name, f.describe_value(key)?);
                let omit_none = f.attrs.omit_none.get(key).copied().unwrap_or(false);
                let omit_empty = f.attrs.omit_empty.get(key).copied().unwrap_or(false);
                if f.attrs.omit_if.get(key).is_some() {
                    return Ok(quote!(
                        return ::core::result::Result::Err(
                            __cx.unsupported_attribute(::core::any::type_name::<Self>(), "omit_if")
                        )
                    ));
                }
                let (q, d) = f.quantization(key);
                items.push(quote!((#name, ::quiddity::identity::export::field_quantized(#kind, #omit_none, #omit_empty, #q, #d))));
            }
            if !flattened.is_empty() {
                return Ok(quote!({
                    let __fields = ::quiddity::__private::vec![#(#items),*];
                    let __flattened = ::quiddity::__private::vec![#(#flattened),*];
                    ::quiddity::identity::export::object_flattened(__cx, __fields, __flattened)?
                }));
            }
            Ok(quote!(::quiddity::identity::export::object(
                ::quiddity::__private::vec![#(#items),*],
                false
            )))
        }
    }
}

/// `kind`, an expression giving a kind, evaluated with `step` added to the
/// exporter's path for errors.
fn within(step: &str, kind: TokenStream) -> TokenStream {
    quote!(__cx.within(#step, |__cx| ::core::result::Result::Ok(#kind))?)
}

impl Field {
    /// The field's `quantize` and `decimals`, as `Option` expressions.
    fn quantization(&self, key: &str) -> (TokenStream, TokenStream) {
        let opt = |v: Option<TokenStream>| match v {
            Some(v) => quote!(::core::option::Option::Some(#v)),
            None => quote!(::core::option::Option::None),
        };
        (
            opt(self.attrs.quantize.get(key).map(|q| quote!(#q))),
            opt(self.attrs.decimals.get(key).map(|d| quote!(#d))),
        )
    }

    /// Whether the field is quantized under `key`, which an identity can
    /// express only for fields with names.
    fn is_quantized(&self, key: &str) -> bool {
        self.attrs.quantize.get(key).is_some() || self.attrs.decimals.get(key).is_some()
    }

    /// An expression giving the kind of the field's value.
    fn describe_value(&self, key: &str) -> syn::Result<TokenStream> {
        let ty = &self.ty;
        let unsupported = |what: &str| {
            quote!(return ::core::result::Result::Err(
                __cx.unsupported_attribute(::core::any::type_name::<Self>(), #what)
            ))
        };
        Ok(match self.encoding(key)? {
            ValueEncoding::Own(j) => quote!(__cx.describe::<#ty, #j>()?),
            ValueEncoding::Quantize(_) | ValueEncoding::Decimals(_) => {
                quote!(__cx.describe::<#ty, ::quiddity::Primary>()?)
            }
            ValueEncoding::Set => quote!({
                let __kind = __cx.describe::<#ty, ::quiddity::Primary>()?;
                ::quiddity::identity::export::as_set(__cx, __kind, ::core::any::type_name::<#ty>())?
            }),
            ValueEncoding::With(_) => unsupported("with"),
            ValueEncoding::SerializeWith(_) => unsupported("serialize_with"),
            ValueEncoding::Display => unsupported("encode_as = display"),
            ValueEncoding::Serde => unsupported("encode_as = serde"),
        })
    }
}

/// An explicit discriminant as an `i128` expression.
fn discriminant(expr: &Expr) -> TokenStream {
    let literal = match expr {
        Expr::Lit(ExprLit {
            lit: Lit::Int(i), ..
        }) => i.base10_parse::<i128>().ok(),
        Expr::Unary(ExprUnary {
            op: UnOp::Neg(_),
            expr,
            ..
        }) => match &**expr {
            Expr::Lit(ExprLit {
                lit: Lit::Int(i), ..
            }) => i.base10_parse::<i128>().ok().map(|n| -n),
            _ => None,
        },
        _ => None,
    };
    match literal {
        Some(n) => quote!(#n),
        None => quote!(((#expr) as i128)),
    }
}

/// Where-clause predicates for the impl: `T: Quiddity<J>` for each type
/// parameter `T` used by a field written through `Quiddity<J>`, outside
/// `PhantomData`.
struct Bounds<'a> {
    input: &'a DeriveInput,
    params: Vec<Ident>,
    predicates: Vec<TokenStream>,
    seen: BTreeSet<String>,
}

impl<'a> Bounds<'a> {
    fn new(input: &'a DeriveInput) -> Self {
        let params = input
            .generics
            .type_params()
            .map(|p| p.ident.clone())
            .collect();
        Bounds {
            input,
            params,
            predicates: Vec::new(),
            seen: BTreeSet::new(),
        }
    }

    fn add(&mut self, ty: &Type, identity: &TokenStream) {
        struct Find<'p> {
            params: &'p [Ident],
            found: Vec<Ident>,
        }
        impl<'ast> Visit<'ast> for Find<'_> {
            fn visit_path(&mut self, path: &'ast syn::Path) {
                if path
                    .segments
                    .last()
                    .is_some_and(|s| s.ident == "PhantomData")
                {
                    return;
                }
                if let Some(ident) = path.get_ident() {
                    if self.params.contains(ident) && !self.found.contains(ident) {
                        self.found.push(ident.clone());
                    }
                }
                syn::visit::visit_path(self, path);
            }
        }
        let mut find = Find {
            params: &self.params,
            found: Vec::new(),
        };
        find.visit_type(ty);
        for param in find.found {
            let predicate = quote!(#param: ::quiddity::Quiddity<#identity>);
            if self.seen.insert(predicate.to_string()) {
                self.predicates.push(predicate);
            }
        }
    }

    fn where_clause(&self) -> TokenStream {
        let existing = self
            .input
            .generics
            .where_clause
            .as_ref()
            .map(|w| &w.predicates);
        let predicates = &self.predicates;
        if existing.is_none_or(|e| e.is_empty()) && predicates.is_empty() {
            return quote!();
        }
        let existing = existing.into_iter();
        quote!(where #(#existing,)* #(#predicates,)*)
    }
}
