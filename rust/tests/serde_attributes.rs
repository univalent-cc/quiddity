//! `#[quiddity(serde_attributes)]` (D21): serde's attributes read as their
//! quiddity equivalents. Where the field types encode alike, the derived
//! digest equals the digest of the serde form (through the serde bridge).

#![cfg(all(feature = "std", feature = "derive", feature = "serde"))]

use std::collections::BTreeMap;

use quiddity::{Digest, IdentityMarker, Quiddity, QuiddityExt};
use serde::Serialize;

/// The digest of the value's serde form.
fn serde_digest<T: Serialize>(value: &T) -> Digest {
    quiddity::serde::digest(value).unwrap()
}

#[derive(Serialize, Quiddity)]
#[serde(rename_all = "camelCase")]
#[quiddity(serde_attributes)]
struct Question {
    brief_text: String,
    #[serde(rename = "kind")]
    question_kind: String,
    #[serde(skip)]
    cache_hits: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    note: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    tags: Vec<String>,
    #[serde(default, alias = "old")]
    priority: u8,
}

fn question(note: Option<&str>, tags: &[&str]) -> Question {
    Question {
        brief_text: "b".into(),
        question_kind: "q".into(),
        cache_hits: 7,
        note: note.map(Into::into),
        tags: tags.iter().map(|t| t.to_string()).collect(),
        priority: 1,
    }
}

#[test]
fn renames_skips_and_omissions() {
    for q in [
        question(None, &[]),
        question(Some("n"), &[]),
        question(None, &["a", "b"]),
    ] {
        assert_eq!(q.quiddity(), serde_digest(&q));
    }
}

#[test]
fn enums() {
    #[derive(Serialize, Quiddity)]
    #[serde(
        tag = "type",
        rename_all = "snake_case",
        rename_all_fields = "camelCase"
    )]
    #[quiddity(serde_attributes)]
    enum Shape {
        UnitSquare,
        Rect {
            side_a: u32,
            side_b: u32,
        },
        #[serde(rename = "circle", rename_all = "UPPERCASE")]
        Round {
            radius: u32,
        },
    }
    for s in [
        Shape::UnitSquare,
        Shape::Rect {
            side_a: 1,
            side_b: 2,
        },
        Shape::Round { radius: 3 },
    ] {
        assert_eq!(s.quiddity(), serde_digest(&s));
    }

    #[derive(Serialize, Quiddity)]
    #[serde(tag = "t", content = "c")]
    #[quiddity(serde_attributes)]
    enum Adjacent {
        A(u32),
        B { x: u8 },
    }
    for a in [Adjacent::A(1), Adjacent::B { x: 2 }] {
        assert_eq!(a.quiddity(), serde_digest(&a));
    }

    #[derive(Serialize, Quiddity)]
    #[serde(untagged)]
    #[quiddity(serde_attributes)]
    enum Untagged {
        Number(u32),
        Pair { a: u8, b: u8 },
    }
    for u in [Untagged::Number(1), Untagged::Pair { a: 1, b: 2 }] {
        assert_eq!(u.quiddity(), serde_digest(&u));
    }
}

/// serde writes a struct's name, or its `rename`, under its `tag`.
/// The serialize side of a split rename; deserialize-only attributes and a
/// variant's `skip` are ignored.
#[test]
fn split_renames_and_ignored() {
    #[derive(Serialize, Quiddity)]
    #[serde(
        rename_all(serialize = "camelCase", deserialize = "snake_case"),
        deny_unknown_fields
    )]
    #[quiddity(serde_attributes)]
    struct Split {
        #[serde(rename(serialize = "out", deserialize = "in"))]
        renamed: u8,
        #[serde(rename(deserialize = "in_only"))]
        read_name: u8,
        two_words: u8,
    }
    let s = Split {
        renamed: 1,
        read_name: 2,
        two_words: 3,
    };
    assert_eq!(s.quiddity(), serde_digest(&s));

    #[derive(Serialize, Quiddity)]
    #[quiddity(serde_attributes)]
    enum Shape {
        Circle(u32),
        #[serde(skip_serializing)]
        Secret(u32),
    }
    assert_eq!(Shape::Circle(1).quiddity(), serde_digest(&Shape::Circle(1)));
    // serde cannot serialize it; it is digested as if not skipped.
    assert!(quiddity::serde::digest(&Shape::Secret(1)).is_err());
    #[derive(Quiddity)]
    enum Plain {
        Secret(u32),
    }
    assert_eq!(Shape::Secret(1).quiddity(), Plain::Secret(1).quiddity());
}

#[test]
fn struct_tag() {
    #[derive(Serialize, Quiddity)]
    #[serde(tag = "type", rename = "query")]
    #[quiddity(serde_attributes)]
    struct Query {
        term: String,
    }
    let q = Query { term: "t".into() };
    assert_eq!(q.quiddity(), serde_digest(&q));

    #[derive(Serialize, Quiddity)]
    #[serde(tag = "type")]
    #[quiddity(serde_attributes)]
    struct Plain {
        term: String,
    }
    let p = Plain { term: "t".into() };
    assert_eq!(p.quiddity(), serde_digest(&p));

    // With `rename_all`, which does not apply to the tag, and a flattened
    // field: {"type": "Flat", "TERM": "t", "n": 1}.
    #[derive(Serialize, Quiddity)]
    #[quiddity(serde_attributes)]
    struct Page {
        n: u32,
    }
    #[derive(Serialize, Quiddity)]
    #[serde(tag = "type", rename_all = "UPPERCASE")]
    #[quiddity(serde_attributes)]
    struct Flat {
        term: String,
        #[serde(flatten)]
        page: Page,
    }
    let f = Flat {
        term: "t".into(),
        page: Page { n: 1 },
    };
    assert_eq!(f.quiddity(), serde_digest(&f));

    // An empty struct is its tag alone: {"type": "Ping"}.
    #[derive(Serialize, Quiddity)]
    #[serde(tag = "type")]
    #[quiddity(serde_attributes)]
    struct Ping {}
    assert_eq!(Ping {}.quiddity(), serde_digest(&Ping {}));
}

#[test]
fn transparent_and_flatten() {
    #[derive(Serialize, Quiddity)]
    #[serde(transparent)]
    #[quiddity(serde_attributes)]
    struct Id(u64);

    #[derive(Serialize, Quiddity)]
    #[quiddity(serde_attributes)]
    struct Page {
        number: u32,
    }

    #[derive(Serialize, Quiddity)]
    #[quiddity(serde_attributes)]
    struct Query {
        id: Id,
        #[serde(flatten)]
        page: Page,
        #[serde(flatten)]
        rest: BTreeMap<String, u32>,
    }
    let q = Query {
        id: Id(9),
        page: Page { number: 2 },
        rest: BTreeMap::from([("size".into(), 20)]),
    };
    assert_eq!(q.quiddity(), serde_digest(&q));
}

mod as_text {
    pub fn serialize<S: serde::Serializer>(n: &u32, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&n.to_string())
    }
}

fn upper<S: serde::Serializer>(t: &str, s: S) -> Result<S::Ok, S::Error> {
    s.serialize_str(&t.to_uppercase())
}

#[test]
fn serialize_with() {
    #[derive(Serialize, Quiddity)]
    #[quiddity(serde_attributes)]
    struct Item {
        #[serde(with = "as_text")]
        count: u32,
        #[serde(serialize_with = "upper")]
        name: String,
    }
    let item = Item {
        count: 5,
        name: "a".into(),
    };
    assert_eq!(item.quiddity(), serde_digest(&item));
}

/// `into` and a variant serde does not tag as the others: the type is
/// digested through its `Serialize` impl.
#[test]
fn through_serialize() {
    #[derive(Clone, Serialize, Quiddity)]
    #[serde(into = "String")]
    #[quiddity(serde_attributes)]
    struct Name(String);
    impl From<Name> for String {
        fn from(n: Name) -> String {
            format!("<{}>", n.0)
        }
    }
    let n = Name("a".into());
    assert_eq!(n.quiddity(), serde_digest(&n));
    assert_eq!(n.quiddity(), "<a>".quiddity());

    #[derive(Serialize, Quiddity)]
    #[quiddity(serde_attributes)]
    enum Partly {
        Tagged(u32),
        #[serde(untagged)]
        Other(String),
    }
    for p in [Partly::Tagged(1), Partly::Other("x".into())] {
        assert_eq!(p.quiddity(), serde_digest(&p));
    }
}

/// Quiddity's own attributes that serde cannot express mix in.
#[test]
fn quantize_mixes_in() {
    #[derive(Serialize, Quiddity)]
    #[quiddity(serde_attributes, domain = "example.reading")]
    struct Reading {
        #[serde(rename = "v")]
        #[quiddity(decimals = 1)]
        value: f64,
    }
    #[derive(Quiddity)]
    #[quiddity(domain = "example.reading")]
    struct Plain {
        v: f64,
    }
    assert_eq!(
        Reading { value: 1.27 }.quiddity(),
        Plain { v: 1.2 }.quiddity()
    );
}

#[derive(IdentityMarker)]
pub struct Meaning;

/// The mode applies to the named identities. The others do not see
/// serde's attributes, and may use quiddity's, scoped to them.
#[test]
fn scoped_to_identities() {
    #[derive(Serialize, Quiddity)]
    #[quiddity(identities(Meaning), serde_attributes(Primary))]
    struct Task {
        #[serde(rename = "p")]
        prompt: String,
        #[quiddity(skip(Meaning))]
        priority: u8,
    }
    #[derive(Quiddity)]
    struct Wire {
        p: String,
        priority: u8,
    }
    #[derive(Quiddity)]
    struct Rust {
        prompt: String,
    }
    let t = Task {
        prompt: "x".into(),
        priority: 1,
    };
    assert_eq!(
        t.quiddity(),
        Wire {
            p: "x".into(),
            priority: 1
        }
        .quiddity()
    );
    assert_eq!(
        t.quiddity_as::<Meaning>(),
        Rust { prompt: "x".into() }.quiddity()
    );
}

/// The standard predicates map to omissions an identity document can hold;
/// another function maps to `omit_if`, which it cannot.
#[cfg(feature = "identity")]
#[test]
fn export() {
    let identity = <Question as Quiddity>::construct_identity().unwrap();
    let q = question(Some("n"), &["a"]);
    assert_eq!(identity.digest(&q).unwrap(), q.quiddity());
    let q = question(None, &[]);
    assert_eq!(identity.digest(&q).unwrap(), q.quiddity());

    fn is_zero(n: &u32) -> bool {
        *n == 0
    }
    #[derive(Serialize, Quiddity)]
    #[quiddity(serde_attributes)]
    struct Counter {
        #[serde(skip_serializing_if = "is_zero")]
        n: u32,
    }
    assert_eq!(Counter { n: 0 }.quiddity(), serde_digest(&Counter { n: 0 }));
    assert_eq!(Counter { n: 1 }.quiddity(), serde_digest(&Counter { n: 1 }));
    let err = <Counter as Quiddity>::construct_identity().unwrap_err();
    assert!(err.to_string().contains("omit_if"), "{err}");
}
