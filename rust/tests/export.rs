//! Exported identities (spec 6.8): a value's derived digest equals the
//! digest of its serde JSON form under its type's exported identity; and
//! the enum tagging styles (spec 6.4) match serde's.

#![cfg(all(feature = "identity", feature = "serde"))]

use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::net::IpAddr;
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use quiddity::identity::Identity;
use quiddity::{IdentityMarker, Primary, Quiddity, QuiddityExt};
use serde::Serialize;

#[derive(IdentityMarker)]
pub struct Meaning;

/// The derived digest under `I` equals the digest of the serde JSON form
/// under the exported identity, and the serde bridge agrees.
fn check<T, I>(value: &T)
where
    T: Quiddity<I> + Serialize,
    I: IdentityMarker,
{
    let identity = <T as Quiddity<I>>::construct_identity().unwrap_or_else(|e| panic!("{e}"));
    let json = serde_json::to_value(value).unwrap();
    let dynamic = identity
        .digest(&json)
        .unwrap_or_else(|e| panic!("{e}\n{json}\n{}", identity.to_json()));
    assert_eq!(
        value.quiddity_as::<I>(),
        dynamic,
        "\n{json}\n{:#}",
        identity.to_json()
    );
}

#[derive(Serialize, Quiddity)]
#[serde(rename_all = "camelCase")]
#[quiddity(
    identities(Meaning),
    rename_all = "camelCase",
    domain = "example.question"
)]
struct Question {
    brief: String,
    rows: Vec<Row>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[quiddity(omit_none)]
    note: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    #[quiddity(omit_empty)]
    attachments: Vec<String>,
    #[quiddity(encode_as = set)]
    tags: Vec<String>,
    labels: BTreeSet<String>,
    asked_at: SystemTime,
    timeout: Duration,
    weights: BTreeMap<u32, f64>,
    shape: Shape,
    #[quiddity(skip(Meaning))]
    priority: u8,
    #[quiddity(identity = Meaning)]
    parent: Option<Box<Question>>,
}

#[derive(Serialize, Quiddity)]
#[quiddity(identities(Meaning))]
struct Row {
    label: String,
    value: f64,
    pair: (u8, String),
    #[quiddity(skip(Meaning))]
    host: IpAddr,
    path: PathBuf,
}

#[derive(Serialize, Quiddity)]
#[serde(rename_all = "snake_case")]
#[quiddity(identities(Meaning), rename_all = "snake_case")]
enum Shape {
    Empty,
    Circle(f64),
    Rect(u32, u32),
    Poly { sides: u8, closed: bool },
}

fn question(parent: Option<Box<Question>>) -> Question {
    Question {
        brief: "b".into(),
        rows: vec![Row {
            label: "x".into(),
            value: 1.5,
            pair: (1, "a".into()),
            host: "192.0.2.1".parse().unwrap(),
            path: "a/b".into(),
        }],
        note: None,
        attachments: vec![],
        tags: vec!["b".into(), "a".into()],
        labels: BTreeSet::from(["l".into()]),
        asked_at: UNIX_EPOCH + Duration::new(1_704_067_200, 5),
        timeout: Duration::from_millis(1500),
        weights: BTreeMap::from([(2, 0.5), (10, 1.0)]),
        shape: Shape::Poly {
            sides: 3,
            closed: true,
        },
        priority: 1,
        parent,
    }
}

#[test]
fn derived_structs() {
    let q = question(None);
    check::<_, Primary>(&q);
    check::<_, Meaning>(&q);
    let mut q = question(Some(Box::new(question(None))));
    q.note = Some("n".into());
    q.attachments = vec!["f".into()];
    q.shape = Shape::Rect(1, 2);
    check::<_, Primary>(&q);
    check::<_, Meaning>(&q);
    for shape in [Shape::Empty, Shape::Circle(0.5)] {
        check::<_, Primary>(&shape);
    }
}

#[test]
fn exported_document() {
    let id = <Question as Quiddity>::construct_identity().unwrap();
    assert_eq!(id.domain(), Some("example.question"));
    let doc = id.to_json();
    let defs = doc["definitions"].as_object().unwrap();
    // One definition per type and identity reached.
    let mut names: Vec<&str> = defs.keys().map(String::as_str).collect();
    names.sort();
    // Field values are under their types' primary identity unless the field
    // says otherwise, as `parent` does.
    assert_eq!(names, ["Question", "Question/Meaning", "Row", "Shape"]);
    assert_eq!(doc["root"], serde_json::json!({"ref": "Question"}));
    let fields = &defs["Question"]["object"]["fields"];
    assert_eq!(
        fields["askedAt"]["kind"],
        serde_json::json!({"instant": {}})
    );
    assert_eq!(fields["tags"]["kind"], serde_json::json!({"set": "string"}));
    assert_eq!(
        fields["weights"]["kind"],
        serde_json::json!({"map": {"key": "integer", "value": "number"}})
    );
    assert!(
        defs["Question/Meaning"]["object"]["fields"]
            .get("priority")
            .is_none()
    );
}

#[derive(Serialize, Quiddity)]
#[serde(tag = "type")]
#[quiddity(tag = "type")]
enum Internal {
    Empty,
    Rect { w: u32, h: u32 },
}

#[derive(Serialize, Quiddity)]
#[serde(tag = "t", content = "c")]
#[quiddity(tag = "t", content = "c")]
enum Adjacent {
    Empty,
    Circle(f64),
    Rect(u32, u32),
    Poly { sides: u8 },
}

#[derive(Serialize, Quiddity)]
#[serde(untagged)]
#[quiddity(untagged)]
enum Untagged {
    Nothing,
    Id(u64),
    Name(String),
    Point { x: i32, y: i32 },
    Pair(bool, bool),
}

#[test]
fn tagging_styles() {
    let internal = [Internal::Empty, Internal::Rect { w: 1, h: 2 }];
    let adjacent = [
        Adjacent::Empty,
        Adjacent::Circle(1.5),
        Adjacent::Rect(1, 2),
        Adjacent::Poly { sides: 3 },
    ];
    let untagged = [
        Untagged::Nothing,
        Untagged::Id(5),
        Untagged::Name("5".into()),
        Untagged::Point { x: 1, y: -1 },
        Untagged::Pair(true, false),
    ];
    for v in &internal {
        check::<_, Primary>(v);
        assert_eq!(quiddity::serde::digest(v).unwrap(), v.quiddity());
    }
    for v in &adjacent {
        check::<_, Primary>(v);
        assert_eq!(quiddity::serde::digest(v).unwrap(), v.quiddity());
    }
    for v in &untagged {
        check::<_, Primary>(v);
        assert_eq!(quiddity::serde::digest(v).unwrap(), v.quiddity());
    }
    // `5` and `"5"` stay apart.
    assert_ne!(
        Untagged::Id(5).quiddity(),
        Untagged::Name("5".into()).quiddity()
    );
}

/// Objects told apart by their keys (spec 5.7), as in a TypeScript union
/// `{x, y} | {w, h} | {r, label?} | string`.
#[derive(Serialize, Quiddity)]
#[serde(untagged)]
#[quiddity(untagged)]
enum Figure {
    Point { x: i32, y: i32 },
    Rect(Rect),
    Circle { r: f64, label: Option<String> },
    Named(String),
}

#[derive(Serialize, Quiddity)]
struct Rect {
    w: u32,
    h: u32,
}

#[test]
fn objects_by_keys() {
    let figures = [
        Figure::Point { x: 1, y: 2 },
        Figure::Rect(Rect { w: 3, h: 4 }),
        Figure::Circle {
            r: 0.5,
            label: None,
        },
        Figure::Circle {
            r: 0.5,
            label: Some("c".into()),
        },
        Figure::Named("n".into()),
    ];
    for v in &figures {
        check::<_, Primary>(v);
        assert_eq!(quiddity::serde::digest(v).unwrap(), v.quiddity());
    }
}

/// Two objects that one object could fit: `{w, h}` fits both, since `h` is
/// optional in `Square`. The syntax does not show it.
#[derive(Quiddity)]
#[quiddity(untagged)]
#[allow(dead_code)]
enum Overlapping {
    Rect(Rect),
    Square { w: u32, h: Option<u32> },
}

/// Variants whose values coincide merge in the exported union, as they
/// already share digests: `Small(5)` and `Large(5)` are both the integer 5.
#[derive(Serialize, Quiddity)]
#[serde(untagged)]
#[quiddity(untagged)]
enum Widths {
    Small(u8),
    Large(u64),
}

#[derive(Quiddity)]
struct Custom {
    #[quiddity(with = write_raw)]
    raw: [u8; 2],
}

fn write_raw(v: &[u8; 2], e: &mut quiddity::Encoder) {
    e.bytes(v);
}

#[derive(Quiddity)]
#[quiddity(encode_as = display)]
struct Shown(u8);

impl std::fmt::Display for Shown {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

#[derive(Quiddity)]
struct Keyed {
    by_row: BTreeMap<Vec<u8>, u8>,
}

#[test]
fn export_errors() {
    let err = |r: Result<Identity, quiddity::identity::ExportError>| r.unwrap_err().message;
    assert!(err(<Custom as Quiddity>::construct_identity()).contains("`with`"));
    assert!(err(<Shown as Quiddity>::construct_identity()).contains("encode_as = display"));
    assert!(err(<Overlapping as Quiddity>::construct_identity()).contains("both fit"));
    check::<_, Primary>(&Widths::Small(5));
    check::<_, Primary>(&Widths::Large(500));
    assert_eq!(Widths::Small(5).quiddity(), Widths::Large(5).quiddity());
    assert!(err(<Keyed as Quiddity>::construct_identity()).contains("key"));
    let infallible = err(<std::convert::Infallible as Quiddity>::construct_identity());
    assert!(
        infallible.contains("no identity document form"),
        "{infallible}"
    );
}

#[test]
fn std_types() {
    check::<_, Primary>(&(1u8, "a", Some(2.5f64), None::<bool>));
    check::<_, Primary>(&HashSet::from([1u16, 2, 3]));
    check::<_, Primary>(&Ok::<u8, String>(1));
    check::<_, Primary>(&Err::<u8, String>("e".into()));
    check::<_, Primary>(&std::ops::Bound::Included(3u8));
    check::<_, Primary>(&std::ops::Bound::<u8>::Unbounded);
    check::<_, Primary>(&(2u8..5));
    check::<_, Primary>(&(2u8..=5));
    check::<_, Primary>(&std::ffi::CString::new("c").unwrap());
    check::<_, Primary>(&std::ffi::OsString::from("os"));
    check::<_, Primary>(&'x');
    check::<_, Primary>(&std::num::NonZero::new(7u32).unwrap());
    check::<_, Primary>(&serde_json::json!({"any": [1, "two", null]}));
}

/// The identity of identity documents, exported from the `Identity` type,
/// gives every identity's own digest when applied to its document.
#[test]
fn identity_exports_itself() {
    let meta = <Identity as Quiddity>::construct_identity().unwrap();
    assert_eq!(meta.domain(), Some("quiddity.identity"));
    for id in [
        meta.clone(),
        <Question as Quiddity<Meaning>>::construct_identity().unwrap(),
        <Untagged as Quiddity>::construct_identity().unwrap(),
    ] {
        assert_eq!(meta.digest(&id.to_json()).unwrap(), id.quiddity());
    }
}
