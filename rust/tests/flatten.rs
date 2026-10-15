//! `flatten` (D21): a field's members are written into the containing
//! map, so that the digest is that of the flattened object.

#![cfg(all(feature = "std", feature = "derive"))]

use std::collections::BTreeMap;

use quiddity::{IdentityMarker, Quiddity, QuiddityExt};

#[derive(Quiddity, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
struct Page {
    number: u32,
    size: u32,
}

#[derive(Quiddity)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
struct Query {
    term: String,
    #[quiddity(flatten)]
    #[cfg_attr(feature = "serde", serde(flatten))]
    page: Page,
}

/// The same members, written out.
#[derive(Quiddity)]
struct Flat {
    term: String,
    number: u32,
    size: u32,
}

fn query() -> Query {
    Query {
        term: "report".into(),
        page: Page {
            number: 2,
            size: 20,
        },
    }
}

fn flat() -> Flat {
    Flat {
        term: "report".into(),
        number: 2,
        size: 20,
    }
}

#[test]
fn struct_members_join_the_parent() {
    assert_eq!(query().quiddity(), flat().quiddity());
}

/// The digest is that of serde's flattened form.
#[cfg(feature = "serde")]
#[test]
fn as_serde_flattens() {
    assert_eq!(
        quiddity::serde::digest(&query()).unwrap(),
        query().quiddity()
    );
}

#[test]
fn nested_and_optional() {
    #[derive(Quiddity)]
    struct Outer {
        #[quiddity(flatten)]
        query: Query,
        #[quiddity(flatten)]
        extra: Option<Page>,
    }
    #[derive(Quiddity)]
    struct Paged {
        term: String,
        #[quiddity(flatten)]
        page: Page,
    }
    let none = Outer {
        query: query(),
        extra: None,
    };
    assert_eq!(none.quiddity(), flat().quiddity());
    // A flattened `Some` adds its members; here, the ones the query has.
    let paged = Paged {
        term: "report".into(),
        page: Page {
            number: 2,
            size: 20,
        },
    };
    assert_eq!(paged.quiddity(), flat().quiddity());
}

#[test]
fn maps_join_the_parent() {
    #[derive(Quiddity)]
    struct Open {
        term: String,
        #[quiddity(flatten)]
        rest: BTreeMap<String, u32>,
    }
    let open = Open {
        term: "report".into(),
        rest: BTreeMap::from([("number".into(), 2), ("size".into(), 20)]),
    };
    assert_eq!(open.quiddity(), flat().quiddity());
}

#[test]
fn generic_fields() {
    #[derive(Quiddity)]
    struct Wrapper<T> {
        term: String,
        #[quiddity(flatten)]
        inner: T,
    }
    let w = Wrapper {
        term: "report".into(),
        inner: Page {
            number: 2,
            size: 20,
        },
    };
    assert_eq!(w.quiddity(), flat().quiddity());
}

#[test]
fn enums_by_tagging_style() {
    #[derive(Quiddity)]
    #[quiddity(tag = "type")]
    enum Internal {
        Circle { r: u32 },
        Empty,
    }
    #[derive(Quiddity)]
    enum External {
        Circle { r: u32 },
    }
    #[derive(Quiddity)]
    #[quiddity(tag = "t", content = "c")]
    enum Adjacent {
        Circle(u32),
    }
    #[derive(Quiddity)]
    struct Shaped<S> {
        name: String,
        #[quiddity(flatten)]
        shape: S,
    }
    fn shaped<S>(shape: S) -> Shaped<S> {
        Shaped {
            name: "a".into(),
            shape,
        }
    }
    #[derive(Quiddity)]
    struct InternalCircle {
        name: String,
        r#type: String,
        r: u32,
    }
    assert_eq!(
        shaped(Internal::Circle { r: 3 }).quiddity(),
        InternalCircle {
            name: "a".into(),
            r#type: "Circle".into(),
            r: 3
        }
        .quiddity()
    );
    #[derive(Quiddity)]
    struct InternalEmpty {
        name: String,
        r#type: String,
    }
    assert_eq!(
        shaped(Internal::Empty).quiddity(),
        InternalEmpty {
            name: "a".into(),
            r#type: "Empty".into()
        }
        .quiddity()
    );
    #[derive(Quiddity)]
    struct Radius {
        r: u32,
    }
    #[derive(Quiddity)]
    #[allow(non_snake_case)]
    struct ExternalCircle {
        name: String,
        Circle: Radius,
    }
    assert_eq!(
        shaped(External::Circle { r: 3 }).quiddity(),
        ExternalCircle {
            name: "a".into(),
            Circle: Radius { r: 3 }
        }
        .quiddity()
    );
    #[derive(Quiddity)]
    struct AdjacentCircle {
        name: String,
        t: String,
        c: u32,
    }
    assert_eq!(
        shaped(Adjacent::Circle(3)).quiddity(),
        AdjacentCircle {
            name: "a".into(),
            t: "Circle".into(),
            c: 3
        }
        .quiddity()
    );
}

#[test]
#[should_panic(expected = "the key \"number\" is written twice while digesting")]
fn a_key_written_twice_panics() {
    #[derive(Quiddity)]
    struct Twice {
        number: u32,
        #[quiddity(flatten)]
        page: Page,
    }
    let _ = Twice {
        number: 1,
        page: Page { number: 2, size: 3 },
    }
    .quiddity();
}

#[test]
#[should_panic(expected = "cannot flatten the unit variant `Empty`")]
fn an_external_unit_variant_cannot_be_flattened() {
    #[derive(Quiddity)]
    enum Shape {
        Empty,
    }
    #[derive(Quiddity)]
    struct Shaped {
        #[quiddity(flatten)]
        shape: Shape,
    }
    let _ = Shaped {
        shape: Shape::Empty,
    }
    .quiddity();
}

#[derive(IdentityMarker)]
pub struct Meaning;

/// Scoped as other attributes are: flattened under `Meaning` only.
#[test]
fn scoped_to_an_identity() {
    #[derive(Quiddity)]
    #[quiddity(identities(Meaning))]
    struct Query2 {
        term: String,
        #[quiddity(flatten(Meaning))]
        page: Page,
    }
    #[derive(Quiddity)]
    struct Nested {
        term: String,
        page: Page,
    }
    let q = Query2 {
        term: "report".into(),
        page: Page {
            number: 2,
            size: 20,
        },
    };
    assert_eq!(q.quiddity_as::<Meaning>(), flat().quiddity());
    assert_eq!(
        q.quiddity(),
        Nested {
            term: "report".into(),
            page: q.page.clone()
        }
        .quiddity()
    );
}

/// An exported identity merges a flattened struct's fields, and reads the
/// serde form of the value.
#[cfg(all(feature = "identity", feature = "serde"))]
#[test]
fn export() {
    let identity = <Query as Quiddity>::construct_identity().unwrap();
    let document = serde_json::to_value(&identity).unwrap();
    let fields = &document["definitions"]["Query"]["object"]["fields"];
    assert_eq!(
        fields.as_object().unwrap().keys().collect::<Vec<_>>(),
        ["number", "size", "term"]
    );
    assert_eq!(identity.digest(&query()).unwrap(), query().quiddity());

    // A flattened map cannot be expressed.
    #[derive(Quiddity)]
    struct Open {
        #[quiddity(flatten)]
        rest: BTreeMap<String, u32>,
    }
    let err = <Open as Quiddity>::construct_identity().unwrap_err();
    assert!(
        err.to_string().contains("not a struct with named fields"),
        "{err}"
    );
}
