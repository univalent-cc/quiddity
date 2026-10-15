//! Building and changing identities: nesting identities as definitions,
//! resolving recursive kinds, and checking every result (spec 5.9).

use alloc::borrow::ToOwned;
use alloc::boxed::Box;
use alloc::collections::BTreeMap;
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use super::load::{self, InvalidIdentity, invalid};
use super::{
    Field, Identity, Kind, MapKind, ObjectKind, RefName, Style, TagEncoding, TaggedKind, Variant,
};

type Result<T> = core::result::Result<T, InvalidIdentity>;

/// The name of an identity's own definition when it has no name.
const SELF: &str = "_self";

/// Whether `k` contains a recursive kind outside nested identities, which
/// are complete in themselves.
fn contains_recursive(k: &Kind) -> bool {
    match k {
        Kind::Recursive => true,
        Kind::List(i) | Kind::Set(i) => contains_recursive(i),
        Kind::Tuple(items) | Kind::Union(items) => items.iter().any(contains_recursive),
        Kind::Map(m) => contains_recursive(&m.key) || contains_recursive(&m.value),
        Kind::Object(o) => o.fields.values().any(|f| contains_recursive(&f.kind)),
        Kind::Tagged(t) => t
            .variants
            .values()
            .any(|v| v.kind.as_ref().is_some_and(contains_recursive)),
        _ => false,
    }
}

/// A name based on `base` that is neither a definition in `defs` nor in
/// `taken`.
fn fresh(base: &str, defs: &BTreeMap<String, Kind>, taken: &[&String]) -> String {
    let free = |n: &str| !defs.contains_key(n) && !taken.iter().any(|t| t.as_str() == n);
    if free(base) {
        return base.to_owned();
    }
    (2..)
        .map(|i| format!("{base}_{i}"))
        .find(|n| free(n))
        .unwrap()
}

/// `k` with references renamed by `map`.
fn rename_refs(k: Kind, map: &BTreeMap<String, String>) -> Kind {
    let r = |k: Kind| rename_refs(k, map);
    match k {
        Kind::Ref(name) => match map.get(name.as_str()) {
            Some(to) => Kind::Ref(RefName::new(to.clone())),
            None => Kind::Ref(name),
        },
        Kind::List(i) => Kind::List(Box::new(r(*i))),
        Kind::Set(i) => Kind::Set(Box::new(r(*i))),
        Kind::Tuple(items) => Kind::Tuple(items.into_iter().map(r).collect()),
        Kind::Union(items) => Kind::Union(items.into_iter().map(r).collect()),
        Kind::Map(m) => Kind::Map(MapKind {
            key: Box::new(r(*m.key)),
            value: Box::new(r(*m.value)),
        }),
        Kind::Object(mut o) => {
            for f in o.fields.values_mut() {
                f.kind = r(core::mem::replace(&mut f.kind, Kind::Null));
            }
            Kind::Object(o)
        }
        Kind::Tagged(mut t) => {
            for v in t.variants.values_mut() {
                v.kind = v.kind.take().map(r);
            }
            Kind::Tagged(t)
        }
        k => k,
    }
}

/// `k` with nested identities moved into `defs` and replaced by references
/// to them, and recursive kinds replaced by references to `me`.
fn absorb(k: Kind, defs: &mut BTreeMap<String, Kind>, me: Option<&str>) -> Result<Kind> {
    let mut a = |k: Kind| absorb(k, defs, me);
    Ok(match k {
        Kind::Identity(id) => merge(*id, defs),
        Kind::Recursive => match me {
            Some(name) => Kind::Ref(RefName::new(name.to_owned())),
            None => return Err(invalid("$", "a recursive kind outside an identity")),
        },
        Kind::List(i) => Kind::List(Box::new(a(*i)?)),
        Kind::Set(i) => Kind::Set(Box::new(a(*i)?)),
        Kind::Tuple(items) => Kind::Tuple(items.into_iter().map(&mut a).collect::<Result<_>>()?),
        Kind::Union(items) => Kind::Union(items.into_iter().map(&mut a).collect::<Result<_>>()?),
        Kind::Map(m) => Kind::Map(MapKind {
            key: Box::new(a(*m.key)?),
            value: Box::new(a(*m.value)?),
        }),
        Kind::Object(mut o) => {
            for f in o.fields.values_mut() {
                f.kind = a(core::mem::replace(&mut f.kind, Kind::Null))?;
            }
            Kind::Object(o)
        }
        Kind::Tagged(mut t) => {
            for v in t.variants.values_mut() {
                v.kind = v.kind.take().map(&mut a).transpose()?;
            }
            Kind::Tagged(t)
        }
        k => k,
    })
}

/// Moves the definitions of the nested identity `id` into `defs`, reusing
/// equal definitions and renaming others whose names are taken, and returns
/// the kind standing for `id`: a reference to its own definition if it is
/// named or recursive, else its root.
pub(crate) fn merge(id: Identity, defs: &mut BTreeMap<String, Kind>) -> Kind {
    let mut map: BTreeMap<String, String> = BTreeMap::new();
    for (name, k) in &id.definitions {
        let target = if defs.get(name).is_none_or(|existing| existing == k) {
            name.clone()
        } else {
            let taken: Vec<&String> = map.values().collect();
            fresh(name, defs, &taken)
        };
        map.insert(name.clone(), target);
    }
    for (name, k) in id.definitions {
        let renamed = rename_refs(k, &map);
        defs.entry(map[&name].clone()).or_insert(renamed);
    }
    rename_refs(id.root, &map)
}

/// An identity whose root is `kind`.
pub(crate) fn identity(kind: Kind, domain: Option<String>) -> Result<Identity> {
    let mut defs = BTreeMap::new();
    let root = if contains_recursive(&kind) {
        // The identity refers to itself: its root becomes its own
        // definition. The name is reserved before nested identities are
        // merged, so that none takes it.
        defs.insert(SELF.to_owned(), Kind::Null);
        let k = absorb(kind, &mut defs, Some(SELF))?;
        defs.insert(SELF.to_owned(), k);
        Kind::Ref(RefName::new(SELF.to_owned()))
    } else {
        absorb(kind, &mut defs, None)?
    };
    load::build(root, defs, domain)
}

/// The identity, named `name`: its root becomes its own definition, under
/// that name, so that the name is part of its document.
pub(crate) fn named(mut id: Identity, name: String) -> Identity {
    let own = match &id.root {
        Kind::Ref(own) => Some(own.as_str().to_owned()),
        _ => None,
    };
    if own.as_deref() == Some(name.as_str()) {
        return id;
    }
    let taken: Vec<&String> = Vec::new();
    let mut others = id.definitions.clone();
    if let Some(own) = &own {
        others.remove(own);
    }
    let target = fresh(&name, &others, &taken);
    match own {
        Some(own) => {
            let map = BTreeMap::from([(own, target)]);
            let definitions = core::mem::take(&mut id.definitions);
            id.definitions = definitions
                .into_iter()
                .map(|(n, k)| (map.get(&n).cloned().unwrap_or(n), rename_refs(k, &map)))
                .collect();
            id.root = rename_refs(core::mem::replace(&mut id.root, Kind::Null), &map);
        }
        None => {
            let root = core::mem::replace(&mut id.root, Kind::Ref(RefName::new(target.clone())));
            id.definitions.insert(target, root);
        }
    }
    id
}

/// The definition `name` of `id`, as an identity of its own, named `name`.
pub(crate) fn definition(id: &Identity, name: &str) -> Option<Identity> {
    id.definitions.get(name)?;
    let root = Kind::Ref(RefName::new(name.to_owned()));
    // Only what the definition reaches.
    let mut reached = alloc::collections::BTreeSet::new();
    load::reach(&root, &id.definitions, &mut reached);
    let defs = id
        .definitions
        .iter()
        .filter(|(n, _)| reached.contains(*n))
        .map(|(n, k)| (n.clone(), k.clone()))
        .collect();
    load::build(root, defs, None).ok()
}

/// `k` if it is valid as far as can be told outside an identity: a kind
/// containing a recursive kind is checked when placed in one.
pub(crate) fn checked(k: Kind) -> Result<Kind> {
    if !contains_recursive(&k) {
        let mut defs = BTreeMap::new();
        let root = absorb(k.clone(), &mut defs, None)?;
        load::build(root, defs, None)?;
    }
    Ok(k)
}

pub(crate) fn object<N, F>(fields: impl IntoIterator<Item = (N, F)>, list: bool) -> Result<Kind>
where
    N: Into<String>,
    F: Into<Field>,
{
    let mut o = ObjectKind::default();
    let mut order = Vec::new();
    for (name, field) in fields {
        let name = name.into();
        if o.fields.insert(name.clone(), field.into()).is_some() {
            return Err(invalid("$", format!("the field `{name}` is given twice")));
        }
        order.push(name);
    }
    if list {
        o.order = Some(order);
    }
    checked(Kind::Object(o))
}

pub(crate) fn tagged<N: Into<String>>(
    variants: impl IntoIterator<Item = (N, Option<Kind>)>,
    style: Option<Style>,
    tag: Option<String>,
    content: Option<String>,
) -> Result<Kind> {
    let mut t = TaggedKind {
        style,
        tag,
        content,
        ..TaggedKind::default()
    };
    for (name, kind) in variants {
        let name = name.into();
        let v = Variant {
            kind,
            ..Variant::default()
        };
        if t.variants.insert(name.clone(), v).is_some() {
            return Err(invalid("$", format!("the variant `{name}` is given twice")));
        }
    }
    checked(Kind::Tagged(t))
}

pub(crate) fn by_discriminant<N: AsRef<str>>(
    k: Kind,
    discriminants: impl IntoIterator<Item = (N, i128)>,
) -> Result<Kind> {
    let Kind::Tagged(mut t) = k else {
        return Err(invalid("$", "discriminants apply to tagged unions"));
    };
    for (name, d) in discriminants {
        let v = t
            .variants
            .get_mut(name.as_ref())
            .ok_or_else(|| invalid("$", format!("no variant `{}`", name.as_ref())))?;
        v.discriminant = Some(d);
    }
    t.encode_as = Some(TagEncoding::Int);
    checked(Kind::Tagged(t))
}

/// Changes the object at the root of `id` (`definition` is `None`) or
/// defined as `definition`, with `f`; the result is checked, and `id` is
/// unchanged if it is not valid. `f` is given the definitions, to merge
/// nested identities into, and the name of the identity's own definition,
/// for recursive kinds, which `needs_self` asks to be created.
pub(crate) fn edit_object(
    id: &mut Identity,
    definition: Option<&str>,
    needs_self: bool,
    f: impl FnOnce(&mut ObjectKind, &mut BTreeMap<String, Kind>, Option<&str>) -> Result<()>,
) -> Result<()> {
    let mut root = id.root.clone();
    let mut defs = id.definitions.clone();
    let mut me = match &root {
        Kind::Ref(r) => Some(r.as_str().to_owned()),
        _ => None,
    };
    if needs_self && me.is_none() {
        // The root becomes the identity's own definition.
        let name = fresh(SELF, &defs, &[]);
        defs.insert(
            name.clone(),
            core::mem::replace(&mut root, Kind::Ref(RefName::new(name.clone()))),
        );
        me = Some(name);
    }
    let target = match definition {
        Some(d) => d.to_owned(),
        None => match &me {
            Some(r) => r.clone(),
            None => {
                let Kind::Object(mut o) = root else {
                    return Err(invalid("$.root", "the root is not an object"));
                };
                f(&mut o, &mut defs, me.as_deref())?;
                return replace(id, Kind::Object(o), defs);
            }
        },
    };
    let Some(Kind::Object(o)) = defs.get(&target) else {
        return Err(invalid("$", format!("`{target}` is not a defined object")));
    };
    let mut o = o.clone();
    f(&mut o, &mut defs, me.as_deref())?;
    defs.insert(target, Kind::Object(o));
    replace(id, root, defs)
}

fn replace(id: &mut Identity, root: Kind, defs: BTreeMap<String, Kind>) -> Result<()> {
    *id = load::build(root, defs, id.domain.clone())?;
    Ok(())
}

/// Adds or replaces the field `name` of `o`.
pub(crate) fn set_field(
    o: &mut ObjectKind,
    name: String,
    mut field: Field,
    defs: &mut BTreeMap<String, Kind>,
    me: Option<&str>,
) -> Result<()> {
    field.kind = absorb(field.kind, defs, me)?;
    if o.order.is_some() && !field.omit.is_empty() {
        return Err(invalid(
            "$",
            "a list-encoded object's fields cannot be omitted",
        ));
    }
    if o.fields.insert(name.clone(), field).is_none() {
        if let Some(order) = &mut o.order {
            order.push(name);
        }
    }
    Ok(())
}

/// Whether a field needs the identity's own definition.
pub(crate) fn field_is_recursive(f: &Field) -> bool {
    contains_recursive(&f.kind)
}
