//! `#[derive(Quiddity)]`: one `impl Quiddity<I>` per identity (spec 5.3–5.5).

use std::collections::BTreeSet;

use proc_macro2::{Span, TokenStream};
use quote::{format_ident, quote, quote_spanned};
use syn::spanned::Spanned;
use syn::visit::Visit;
use syn::{
    Data, DeriveInput, Expr, ExprLit, ExprUnary, Fields, Ident, Lit, LitStr, Path, Type, UnOp,
    Variant,
};

use crate::attr::{EncodeAs, FieldAttrs, PRIMARY, TypeAttrs, VariantAttrs};

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
    /// Written as `PhantomData<…>`: never participates (spec 5.3). Other
    /// spellings are caught at run time by `ABSENT_AS_FIELD`.
    phantom: bool,
}

/// How a participating field's value is written.
enum ValueEncoding {
    /// Through the field type's own `Quiddity<J>` impl.
    Own(TokenStream),
    With(Path),
    Display,
    Serde,
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
    let attrs = TypeAttrs::parse(&input.attrs)?;
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
        Data::Struct(data) => Body::Struct(fields(&data.fields, &keys)?, data.fields.clone()),
        Data::Enum(data) => {
            if attrs.transparent.is_set() {
                return Err(syn::Error::new(
                    Span::call_site(),
                    "`transparent` applies to structs only",
                ));
            }
            let mut variants = Vec::new();
            for v in &data.variants {
                variants.push((
                    v,
                    VariantAttrs::parse(&v.attrs, &keys)?,
                    fields(&v.fields, &keys)?,
                ));
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

fn fields(fields: &Fields, keys: &[String]) -> syn::Result<Vec<Field>> {
    let mut out = Vec::new();
    for (i, f) in fields.iter().enumerate() {
        let attrs = FieldAttrs::parse(&f.attrs, keys)?;
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
            let write = write_fields(fields, shape, key, &mut bounds)?;
            let pattern = pattern(quote!(Self), fields, form);
            quote! {
                let #pattern = self;
                #write
            }
        }
        (_, Body::Enum(variants)) => {
            let int = match type_shape {
                None | Some((EncodeAs::String, _)) => false,
                Some((EncodeAs::Int, _)) => true,
                Some((_, span)) => {
                    return Err(syn::Error::new(
                        span,
                        "on an enum, `encode_as` must be `string`, `int`, `display` or `serde`",
                    ));
                }
            };
            write_enum(variants, int, key, &mut bounds)?
        }
    };

    let domain = match attrs.domain.get(key) {
        Some(label) => {
            quote!(const DOMAIN: ::core::option::Option<&'static str> = ::core::option::Option::Some(#label);)
        }
        None => quote!(),
    };
    let (impl_generics, ty_generics, _) = input.generics.split_for_impl();
    let where_clause = bounds.where_clause();
    Ok(quote! {
        #[automatically_derived]
        impl #impl_generics ::quiddity::Quiddity<#id_ty> for #name #ty_generics #where_clause {
            #domain

            #[allow(unused_variables, clippy::needless_borrow)]
            fn encode(&self, __e: &mut ::quiddity::Encoder) {
                #encode
            }
        }
    })
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

    fn encoding(&self, key: &str) -> syn::Result<ValueEncoding> {
        let a = &self.attrs;
        let (with, shape, identity) = (a.with.get(key), a.encode_as.get(key), a.identity.get(key));
        if [with.is_some(), shape.is_some(), identity.is_some()]
            .iter()
            .filter(|b| **b)
            .count()
            > 1
        {
            return Err(syn::Error::new_spanned(
                &self.ty,
                format!("`with`, `encode_as` and `identity` cannot be combined (identity `{key}`)"),
            ));
        }
        Ok(match (with, shape, identity) {
            (Some(path), _, _) => ValueEncoding::With(path.clone()),
            (_, Some((EncodeAs::Display, _)), _) => ValueEncoding::Display,
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
            ValueEncoding::Display => quote!(__e.display(#b);),
            ValueEncoding::Serde => quote!(::quiddity::__encode_serde!(#b, __e);),
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
        let none = self
            .attrs
            .none_as_missing
            .get(key)
            .copied()
            .unwrap_or(false);
        let omit_if = self.attrs.omit_if.get(key);
        if (none || omit_if.is_some()) && shape != Shape::Map {
            return Err(syn::Error::new_spanned(
                &self.ty,
                "`none_as_missing` and `omit_if` apply only to fields encoded as a map",
            ));
        }
        if none {
            cond.extend(
                quote_spanned!(ty.span()=> && !::quiddity::__private::OptionLike::is_none(#b)),
            );
        }
        if let Some(path) = omit_if {
            cond.extend(quote!(&& !#path(#b)));
        }
        Ok(cond)
    }
}

/// Statements writing `fields` in `shape` under the identity `key`.
fn write_fields(
    fields: &[Field],
    shape: Shape,
    key: &str,
    bounds: &mut Bounds,
) -> syn::Result<TokenStream> {
    let participating: Vec<&Field> = fields.iter().filter(|f| f.participates(key)).collect();
    // Check omission attributes even on fields that do not participate.
    for f in fields.iter().filter(|f| f.participates(key)) {
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
            let mut entries = Vec::new();
            for f in participating {
                let name = match f.attrs.rename.get(key) {
                    Some(lit) => lit.value(),
                    None => unraw(f.name.as_ref().unwrap()),
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
            let mut writes = TokenStream::new();
            for (name, f) in entries {
                let cond = f.condition(key, shape)?;
                let value = f.write_value(key, bounds)?;
                let name = LitStr::new(&name, Span::call_site());
                writes.extend(quote!(if #cond { __m.field_with(#name, |__e| { #value }); }));
            }
            Ok(quote! {
                let mut __m = __e.fields();
                #writes
                __m.finish();
            })
        }
    }
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

fn write_enum(
    variants: &[(&Variant, VariantAttrs, Vec<Field>)],
    int: bool,
    key: &str,
    bounds: &mut Bounds,
) -> syn::Result<TokenStream> {
    let unit_only = variants
        .iter()
        .all(|(v, ..)| matches!(v.fields, Fields::Unit));
    let mut names = BTreeSet::new();
    let mut previous: Option<TokenStream> = None;
    let mut arms = TokenStream::new();
    for (v, attrs, fields) in variants {
        let ident = &v.ident;
        let name = match attrs.rename.get(key) {
            Some(lit) => lit.value(),
            None => unraw(ident),
        };
        if !names.insert(name.clone()) {
            return Err(syn::Error::new_spanned(
                v,
                format!("two variants have the name {name:?} under identity `{key}`"),
            ));
        }
        let discriminant = if unit_only {
            quote!((Self::#ident as i128))
        } else {
            match (&v.discriminant, &previous) {
                (Some((_, expr)), _) => discriminant(expr),
                (None, Some(prev)) => quote!((#prev + 1i128)),
                (None, None) => quote!(0i128),
            }
        };
        previous = Some(discriminant.clone());

        let form = &v.fields;
        let pattern = pattern(quote!(Self::#ident), fields, form);
        let shape = match form {
            Fields::Named(_) => Shape::Map,
            Fields::Unnamed(f) if f.unnamed.len() == 1 => Shape::Single,
            _ => Shape::List,
        };
        let name = LitStr::new(&name, Span::call_site());
        let write = match (form, int) {
            (Fields::Unit, false) => quote!(__e.str(#name);),
            (Fields::Unit, true) => quote!(__e.int(#discriminant);),
            (_, false) => {
                let data = write_fields(fields, shape, key, bounds)?;
                quote! {
                    let mut __m = __e.fields();
                    __m.field_with(#name, |__e| { #data });
                    __m.finish();
                }
            }
            (_, true) => {
                let data = write_fields(fields, shape, key, bounds)?;
                quote! {
                    let mut __l = __e.list();
                    __l.item_with(|__e| __e.int(#discriminant));
                    __l.item_with(|__e| { #data });
                    __l.finish();
                }
            }
        };
        arms.extend(quote!(#pattern => { #write }));
    }
    Ok(quote! {
        match self {
            #arms
        }
    })
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
