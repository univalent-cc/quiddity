//! `#[derive(Quiddity)]` (spec 6.3–6.5): each derived digest is compared
//! with the digest of an equivalent value built from std types.

#![cfg(all(feature = "std", feature = "derive"))]

use std::collections::BTreeMap;
use std::marker::PhantomData;

use quiddity::{Encoder, IdentityMarker, Quiddity, QuiddityExt};

#[derive(IdentityMarker)]
pub struct Meaning;

#[derive(IdentityMarker)]
pub struct Routing;

type Writer = Box<dyn Fn(&mut Encoder)>;

/// A map with string keys and values of different types, for comparison.
struct Map(Vec<(&'static str, Writer)>);

impl Map {
    fn new() -> Self {
        Map(Vec::new())
    }

    fn with<T: Quiddity + 'static>(mut self, key: &'static str, value: T) -> Self {
        self.0.push((key, Box::new(move |e| value.encode(e))));
        self
    }
}

impl Quiddity for Map {
    fn encode(&self, e: &mut Encoder) {
        let mut m = e.map();
        for (k, f) in &self.0 {
            m.entry_with(|e| e.str(k), f);
        }
    }
}

#[derive(Quiddity)]
struct Interval {
    start: u64,
    end: u64,
}

#[test]
fn struct_is_a_map() {
    let i = Interval { start: 10, end: 15 };
    assert_eq!(
        i.quiddity().to_string(),
        "sha256:ce5c58de07557be10cb78541577e3812bdb0b96b8af3aa17c4224753841bd989"
    );
    assert_eq!(
        i.quiddity(),
        BTreeMap::from([("start", 10u8), ("end", 15)]).quiddity()
    );
}

#[derive(Quiddity)]
#[quiddity(identities(Meaning, Routing))]
struct Task {
    prompt: String,
    #[quiddity(skip(Meaning))]
    priority: u8,
    #[quiddity(skip(Primary))]
    answer_schema: String,
    #[quiddity(skip)]
    submitted_at: u64,
    #[quiddity(skip, skip(Routing) = false)]
    queue: &'static str,
    #[quiddity(rename = "documents", rename(Meaning) = "docs")]
    document_refs: Vec<String>,
}

fn task() -> Task {
    Task {
        prompt: "Summarize".into(),
        priority: 3,
        answer_schema: "text".into(),
        submitted_at: 1_700_000_000,
        queue: "fast",
        document_refs: vec!["a".into()],
    }
}

#[test]
fn identities_and_scoped_attributes() {
    let t = task();
    let docs = vec![String::from("a")];
    assert_eq!(
        t.quiddity(),
        Map::new()
            .with("prompt", "Summarize")
            .with("priority", 3u8)
            .with("documents", docs.clone())
            .quiddity()
    );
    assert_eq!(
        t.quiddity_as::<Meaning>(),
        Map::new()
            .with("prompt", "Summarize")
            .with("answer_schema", "text")
            .with("docs", docs.clone())
            .quiddity()
    );
    assert_eq!(
        t.quiddity_as::<Routing>(),
        Map::new()
            .with("prompt", "Summarize")
            .with("priority", 3u8)
            .with("answer_schema", "text")
            .with("queue", "fast")
            .with("documents", docs)
            .quiddity()
    );
    // Skipped fields do not affect the digest.
    let mut later = task();
    later.submitted_at += 1;
    assert_eq!(later.quiddity(), t.quiddity());
}

#[derive(Quiddity)]
#[quiddity(identities(Meaning))]
struct Inner {
    a: u8,
    #[quiddity(skip(Meaning))]
    b: u8,
}

#[derive(Quiddity)]
#[quiddity(identities(Meaning))]
struct Outer {
    plain: Inner,
    #[quiddity(identity = Meaning)]
    meaning: Inner,
    #[quiddity(identity(Meaning) = Meaning)]
    scoped: Inner,
}

#[test]
fn nested_identities() {
    let o = Outer {
        plain: Inner { a: 1, b: 2 },
        meaning: Inner { a: 1, b: 2 },
        scoped: Inner { a: 1, b: 2 },
    };
    let full = || Map::new().with("a", 1u8).with("b", 2u8);
    let only_a = || Map::new().with("a", 1u8);
    // A nested value uses its primary identity unless `identity` says
    // otherwise, whatever the enclosing identity.
    assert_eq!(
        o.quiddity(),
        Map::new()
            .with("plain", full())
            .with("meaning", only_a())
            .with("scoped", full())
            .quiddity()
    );
    assert_eq!(
        o.quiddity_as::<Meaning>(),
        Map::new()
            .with("plain", full())
            .with("meaning", only_a())
            .with("scoped", only_a())
            .quiddity()
    );
}

fn as_bytes(v: &[u8; 2], e: &mut Encoder) {
    e.bytes(v);
}

fn is_zero(v: &u32) -> bool {
    *v == 0
}

#[derive(Quiddity)]
struct Custom {
    #[quiddity(with = as_bytes)]
    raw: [u8; 2],
    #[quiddity(encode_as = display)]
    addr: std::net::Ipv4Addr,
    #[quiddity(omit_none)]
    note: Option<String>,
    maybe: Option<u8>,
    #[quiddity(omit_if = is_zero)]
    retries: u32,
    #[quiddity(omit_if = Vec::is_empty)]
    tags: Vec<String>,
}

struct Bytes(&'static [u8]);

impl Quiddity for Bytes {
    fn encode(&self, e: &mut Encoder) {
        e.bytes(self.0);
    }
}

#[test]
fn custom_encodings_and_omission() {
    let c = Custom {
        raw: [1, 2],
        addr: std::net::Ipv4Addr::LOCALHOST,
        note: None,
        maybe: None,
        retries: 0,
        tags: vec![],
    };
    assert_eq!(
        c.quiddity(),
        Map::new()
            .with("raw", Bytes(&[1, 2]))
            .with("addr", "127.0.0.1")
            .with("maybe", ())
            .quiddity()
    );
    let c = Custom {
        note: Some("hi".into()),
        retries: 2,
        tags: vec!["x".into()],
        ..c
    };
    assert_eq!(
        c.quiddity(),
        Map::new()
            .with("raw", Bytes(&[1, 2]))
            .with("addr", "127.0.0.1")
            .with("note", "hi")
            .with("maybe", ())
            .with("retries", 2u8)
            .with("tags", vec!["x"])
            .quiddity()
    );
}

#[derive(Quiddity)]
#[quiddity(encode_as = list)]
struct Positional {
    b: u8,
    #[quiddity(skip)]
    _c: u8,
    a: &'static str,
}

#[derive(Quiddity)]
struct Pair(u8, &'static str);

#[derive(Quiddity)]
struct Newtype(u64);

#[derive(Quiddity)]
struct Unit;

#[derive(Quiddity)]
#[quiddity(transparent)]
struct Wrapped {
    #[quiddity(skip)]
    _cache: u8,
    value: String,
}

#[derive(Quiddity)]
#[quiddity(identities(Meaning), encode_as = list, encode_as(Meaning) = map)]
struct Shapes {
    x: u8,
}

#[test]
fn struct_shapes() {
    assert_eq!(
        Positional {
            b: 1,
            _c: 9,
            a: "x"
        }
        .quiddity(),
        (1u8, "x").quiddity()
    );
    assert_eq!(Pair(1, "x").quiddity(), (1u8, "x").quiddity());
    assert_eq!(Newtype(7).quiddity(), 7u8.quiddity());
    assert_eq!(Unit.quiddity(), ().quiddity());
    assert_eq!(
        Wrapped {
            _cache: 0,
            value: "v".into()
        }
        .quiddity(),
        "v".quiddity()
    );
    assert_eq!(Shapes { x: 1 }.quiddity(), [1u8].quiddity());
    assert_eq!(
        Shapes { x: 1 }.quiddity_as::<Meaning>(),
        Map::new().with("x", 1u8).quiddity()
    );
}

#[derive(Quiddity)]
#[quiddity(identities(Meaning), domain = "example.payment", domain(Meaning) = "example.meaning")]
struct Payment {
    to: &'static str,
    amount: u64,
}

#[test]
fn domains() {
    let p = Payment {
        to: "bob",
        amount: 100,
    };
    assert_eq!(
        p.quiddity().to_string(),
        "sha256:f9104f8b85740d8c6c47c0f5a04bb05d264ee25f9be3d310be66385b298da830"
    );
    let structural = Map::new()
        .with("to", "bob")
        .with("amount", 100u8)
        .quiddity();
    assert_eq!(p.quiddity(), structural.in_domain("example.payment"));
    assert_eq!(
        p.quiddity_as::<Meaning>(),
        structural.in_domain("example.meaning")
    );
    // Nested: the domain plays no part.
    assert_eq!(
        vec![p].quiddity(),
        vec![Map::new().with("to", "bob").with("amount", 100u8)].quiddity()
    );
}

#[derive(Quiddity)]
#[quiddity(identities(Meaning))]
enum Shape {
    Empty,
    #[quiddity(rename = "circle", rename(Meaning) = "round")]
    Circle(u32),
    Rect(u32, u32),
    Poly {
        sides: u8,
        #[quiddity(omit_none)]
        label: Option<String>,
    },
    Ghost(PhantomData<String>),
}

#[test]
fn enums_as_strings() {
    assert_eq!(Shape::Empty.quiddity(), "Empty".quiddity());
    assert_eq!(
        Shape::Circle(2).quiddity(),
        Map::new().with("circle", 2u8).quiddity()
    );
    assert_eq!(
        Shape::Circle(2).quiddity_as::<Meaning>(),
        Map::new().with("round", 2u8).quiddity()
    );
    assert_eq!(
        Shape::Rect(1, 2).quiddity(),
        Map::new().with("Rect", [1u8, 2]).quiddity()
    );
    assert_eq!(
        Shape::Poly {
            sides: 5,
            label: None
        }
        .quiddity(),
        Map::new()
            .with("Poly", Map::new().with("sides", 5u8))
            .quiddity()
    );
    assert_eq!(
        Shape::Ghost(PhantomData).quiddity(),
        Map::new().with("Ghost", ()).quiddity()
    );
}

#[derive(Quiddity)]
#[quiddity(encode_as = int)]
enum Level {
    Low,
    High = 10,
    Higher,
}

#[derive(Quiddity)]
#[quiddity(encode_as = int)]
#[repr(i8)]
enum Event {
    Start = -1,
    Tick(u32),
    Stop { code: u8 } = 5,
    After,
}

#[test]
fn enums_as_ints() {
    assert_eq!(Level::Low.quiddity(), 0u8.quiddity());
    assert_eq!(Level::High.quiddity(), 10u8.quiddity());
    assert_eq!(Level::Higher.quiddity(), 11u8.quiddity());
    assert_eq!(Event::Start.quiddity(), (-1i8).quiddity());
    assert_eq!(Event::Tick(3).quiddity(), (0u8, 3u8).quiddity());
    assert_eq!(
        Event::Stop { code: 1 }.quiddity(),
        (5u8, Map::new().with("code", 1u8)).quiddity()
    );
    assert_eq!(Event::After.quiddity(), 6u8.quiddity());
}

#[derive(Quiddity)]
#[quiddity(encode_as = display)]
struct Version(u8, u8);

impl std::fmt::Display for Version {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}", self.0, self.1)
    }
}

#[test]
fn type_as_display() {
    assert_eq!(Version(1, 2).quiddity(), "1.2".quiddity());
}

/// Not `Quiddity`: generic code must not require it through `PhantomData`.
struct Opaque;

#[derive(Quiddity)]
struct Id<T> {
    raw: u64,
    _type: PhantomData<T>,
}

#[derive(Quiddity)]
struct Generic<T, U> {
    value: T,
    items: Vec<U>,
}

#[derive(Quiddity)]
struct Borrowed<'a> {
    name: &'a str,
}

#[derive(Quiddity)]
struct Node {
    label: String,
    children: Vec<Node>,
}

#[derive(Quiddity)]
struct Keywords {
    r#type: u8,
}

#[test]
fn generics_and_special_fields() {
    let id: Id<Opaque> = Id {
        raw: 1,
        _type: PhantomData,
    };
    assert_eq!(id.quiddity(), Map::new().with("raw", 1u8).quiddity());
    // A generic field that turns out to be `PhantomData` is absent too.
    let g: Generic<PhantomData<Opaque>, u8> = Generic {
        value: PhantomData,
        items: vec![1],
    };
    assert_eq!(g.quiddity(), Map::new().with("items", vec![1u8]).quiddity());
    let g: Generic<&str, u8> = Generic {
        value: "v",
        items: vec![],
    };
    assert_eq!(
        g.quiddity(),
        Map::new()
            .with("value", "v")
            .with("items", Vec::<u8>::new())
            .quiddity()
    );
    let owned = String::from("n");
    assert_eq!(
        Borrowed { name: &owned }.quiddity(),
        Map::new().with("name", "n").quiddity()
    );
    let tree = Node {
        label: "root".into(),
        children: vec![Node {
            label: "leaf".into(),
            children: vec![],
        }],
    };
    let leaf = Map::new()
        .with("label", "leaf")
        .with("children", Vec::<u8>::new());
    assert_eq!(
        tree.quiddity(),
        Map::new()
            .with("label", "root")
            .with("children", vec![leaf])
            .quiddity()
    );
    assert_eq!(
        Keywords { r#type: 1 }.quiddity(),
        Map::new().with("type", 1u8).quiddity()
    );
}

/// An identity marker can be a generic parameter.
fn digest_as<I: IdentityMarker, T: Quiddity<I>>(value: &T) -> quiddity::Digest {
    value.quiddity_as::<I>()
}

#[test]
fn identity_as_generic_parameter() {
    assert_eq!(
        digest_as::<Meaning, _>(&task()),
        task().quiddity_as::<Meaning>()
    );
}

mod spelled {
    use super::*;

    #[derive(Quiddity)]
    #[quiddity(identities(crate::Meaning))]
    pub struct Spelled {
        #[quiddity(skip(Meaning))]
        a: u8,
        #[quiddity(skip(quiddity::Primary))]
        b: u8,
    }

    #[test]
    fn identities_matched_by_name() {
        let s = Spelled { a: 1, b: 2 };
        assert_eq!(s.quiddity(), Map::new().with("a", 1u8).quiddity());
        assert_eq!(
            s.quiddity_as::<Meaning>(),
            Map::new().with("b", 2u8).quiddity()
        );
    }
}

#[derive(Quiddity)]
#[quiddity(transparent)]
struct Transparent<T> {
    value: T,
    #[quiddity(skip)]
    _note: &'static str,
}

#[test]
fn generic_transparent() {
    assert_eq!(
        Transparent {
            value: 5u8,
            _note: ""
        }
        .quiddity(),
        5u8.quiddity()
    );
}

#[derive(Quiddity)]
struct Omissions {
    #[quiddity(omit_empty)]
    tags: Vec<String>,
    #[quiddity(omit_empty)]
    note: String,
    #[quiddity(omit_empty)]
    path: std::path::PathBuf,
    #[quiddity(omit_empty)]
    os: std::ffi::OsString,
    #[quiddity(omit_empty)]
    c: std::ffi::CString,
    #[quiddity(omit_empty, omit_none)]
    extra: Option<BTreeMap<String, u8>>,
}

#[test]
fn omit_empty() {
    let empty = Omissions {
        tags: vec![],
        note: String::new(),
        path: Default::default(),
        os: Default::default(),
        c: Default::default(),
        extra: None,
    };
    assert_eq!(empty.quiddity(), Map::new().quiddity());
    let full = Omissions {
        tags: vec!["x".into()],
        note: "n".into(),
        path: "p".into(),
        os: "o".into(),
        c: std::ffi::CString::new("c").unwrap(),
        extra: Some(BTreeMap::new()),
    };
    assert_eq!(
        full.quiddity(),
        Map::new()
            .with("tags", vec!["x"])
            .with("note", "n")
            .with("path", "p")
            .with("os", "o")
            .with("c", "c")
            .quiddity()
    );
}

#[derive(Quiddity)]
#[quiddity(identities(Meaning))]
struct Tagged<T> {
    #[quiddity(encode_as = set)]
    tags: Vec<T>,
    #[quiddity(encode_as(Meaning) = set)]
    order: Vec<u8>,
}

#[test]
fn encode_as_set() {
    use std::collections::BTreeSet;
    let t = Tagged {
        tags: vec!["b", "a", "b"],
        order: vec![2, 1],
    };
    // Sets are multisets: the repeated "b" is kept.
    let expected = Map::new()
        .with("tags", Multiset(vec!["a", "b", "b"]))
        .with("order", vec![2u8, 1]);
    assert_eq!(t.quiddity(), expected.quiddity());
    assert_ne!(
        t.quiddity(),
        Map::new()
            .with("tags", vec!["b", "a", "b"])
            .with("order", vec![2u8, 1])
            .quiddity()
    );
    // A scoped `encode_as`: a set under `Meaning` only.
    assert_eq!(
        t.quiddity_as::<Meaning>(),
        Map::new()
            .with("tags", Multiset(vec!["a", "b", "b"]))
            .with("order", BTreeSet::from([1u8, 2]))
            .quiddity()
    );
    let u = Tagged {
        tags: vec!["a"],
        order: vec![1, 2],
    };
    assert_eq!(
        u.quiddity(),
        Map::new()
            .with("tags", BTreeSet::from(["a"]))
            .with("order", vec![1u8, 2])
            .quiddity()
    );
}

/// A set that keeps repeated members.
struct Multiset(Vec<&'static str>);

impl Quiddity for Multiset {
    fn encode(&self, e: &mut Encoder) {
        let mut s = e.set();
        for m in &self.0 {
            s.member(m);
        }
    }
}

#[derive(Quiddity)]
#[quiddity(identities(Meaning), rename_all = "camelCase", rename_all(Meaning) = "SCREAMING-KEBAB-CASE")]
struct Renamed {
    user_id: u8,
    #[quiddity(rename = "kept")]
    other_name: u8,
    r#type: u8,
}

#[derive(Quiddity)]
#[quiddity(rename_all = "snake_case")]
enum Activity {
    PageView,
    #[quiddity(rename = "click")]
    ButtonClick,
    FormSubmit {
        form_id: u8,
    },
}

#[test]
fn rename_all() {
    let r = Renamed {
        user_id: 1,
        other_name: 2,
        r#type: 3,
    };
    assert_eq!(
        r.quiddity(),
        Map::new()
            .with("userId", 1u8)
            .with("kept", 2u8)
            .with("type", 3u8)
            .quiddity()
    );
    assert_eq!(
        r.quiddity_as::<Meaning>(),
        Map::new()
            .with("USER-ID", 1u8)
            .with("kept", 2u8)
            .with("TYPE", 3u8)
            .quiddity()
    );
    assert_eq!(Activity::PageView.quiddity(), "page_view".quiddity());
    assert_eq!(Activity::ButtonClick.quiddity(), "click".quiddity());
    // Fields of struct variants keep their names, as in serde.
    assert_eq!(
        Activity::FormSubmit { form_id: 4 }.quiddity(),
        Map::new()
            .with("form_submit", Map::new().with("form_id", 4u8))
            .quiddity()
    );
}

#[derive(Quiddity)]
struct Nulls {
    #[quiddity(omit_none)]
    a: Option<u8>,
    #[quiddity(omit_none)]
    b: Box<Option<u8>>,
    #[quiddity(omit_none)]
    c: (),
    #[quiddity(omit_none)]
    d: Option<Option<u8>>,
}

#[test]
fn omit_none_on_nullable_types() {
    let n = Nulls {
        a: None,
        b: Box::new(None),
        c: (),
        d: Some(None),
    };
    // `Some(None)` is not omitted: only the outer `None` is.
    assert_eq!(n.quiddity(), Map::new().with("d", ()).quiddity());
    let n = Nulls {
        a: Some(1),
        b: Box::new(Some(2)),
        c: (),
        d: None,
    };
    assert_eq!(
        n.quiddity(),
        Map::new().with("a", 1u8).with("b", 2u8).quiddity()
    );
}
