//! Dynamic identities (spec 5): the README's example, and composing and
//! changing identities in code.

#![cfg(feature = "identity")]

use quiddity::QuiddityExt;
use quiddity::identity::{Field, Identity, Kind};
use serde_json::json;

/// An identity's JSON form, through its `Serialize` impl.
fn to_json(id: &Identity) -> serde_json::Value {
    serde_json::to_value(id).unwrap()
}

/// An identity from its JSON form, through its `Deserialize` impl.
fn from_json(doc: &serde_json::Value) -> Result<Identity, serde_json::Error> {
    serde_json::from_value(doc.clone())
}

#[test]
fn readme_example() {
    let identity = from_json(&json!({
        "root": {"object": {"fields": {
            "title": {"kind": "string"},
            "pages": {"kind": "integer"},
            "published": {"kind": {"instant": {}}}
        }}}
    }))
    .unwrap();
    let a = identity
        .digest(&json!({
            "title": "Moby Dick", "pages": 635, "published": "1851-10-18T00:00:00Z",
            "last_opened": "2026-10-14T09:30:00Z"
        }))
        .unwrap();
    let b = identity
        .digest(&json!({
            "title": "Moby Dick", "pages": "635", "published": "1851-10-18T01:00:00+01:00"
        }))
        .unwrap();
    assert_eq!(a, b);
    let err = identity
        .digest(&json!({"title": 5, "pages": 635, "published": "1851-10-18T00:00:00Z"}))
        .unwrap_err();
    assert_eq!(err.path, "$.title");
}

fn row() -> Identity {
    Identity::new(
        Kind::object([
            ("label", Field::new(Kind::String)),
            ("value", Field::new(Kind::Number)),
        ])
        .unwrap(),
    )
    .unwrap()
    .named("Row")
}

fn question() -> Identity {
    let shape = Kind::internally_tagged(
        "type",
        [
            ("Empty", None),
            (
                "Rect",
                Some(Kind::object([("w", Kind::Integer), ("h", Kind::Integer)]).unwrap()),
            ),
        ],
    )
    .unwrap();
    let mut q = Identity::new(
        Kind::object([
            ("brief", Field::new(Kind::String)),
            ("note", Field::optional(Kind::String)),
            ("tags", Field::omit_empty(Kind::set(Kind::String))),
            ("rows", Field::new(Kind::list(row()))),
            ("best", Field::optional(row())),
            ("parent", Field::optional(Kind::recursive())),
            ("kind", Field::new(Kind::String).rename("type")),
            ("shape", Field::new(shape)),
        ])
        .unwrap(),
    )
    .unwrap()
    .named("Question");
    q.set_domain("example.question").unwrap();
    q
}

#[test]
fn composed_identity_matches_its_document() {
    let q = question();
    let doc = to_json(&q);
    // Nested identities became definitions under their names; the same
    // `Row`, nested twice, is defined once; the identity refers to itself.
    assert_eq!(doc["root"], json!({"ref": "Question"}));
    let defs = doc["definitions"].as_object().unwrap();
    let mut names: Vec<&str> = defs.keys().map(String::as_str).collect();
    names.sort();
    assert_eq!(names, ["Question", "Row"]);
    let fields = &defs["Question"]["object"]["fields"];
    assert_eq!(fields["rows"]["kind"], json!({"list": {"ref": "Row"}}));
    assert_eq!(
        fields["parent"]["kind"],
        json!({"union": ["null", {"ref": "Question"}]})
    );
    assert_eq!(fields["tags"]["omit"], json!(["none", "empty"]));
    // The JSON form reads back as the same identity, with the same digest.
    let loaded = from_json(&doc).unwrap();
    assert_eq!(loaded, q);
    assert_eq!(loaded.quiddity(), q.quiddity());
    // And it digests values.
    let value = json!({
        "brief": "b", "rows": [{"label": "x", "value": 1}], "kind": "q",
        "shape": {"type": "Rect", "w": 1, "h": 2},
        "parent": {"brief": "p", "rows": [], "kind": "q", "shape": {"type": "Empty"}}
    });
    assert_eq!(q.digest(&value).unwrap(), loaded.digest(&value).unwrap());
}

#[test]
fn definitions_as_identities() {
    let q = question();
    let r = q.definition("Row").unwrap();
    assert_eq!(r, row());
    assert_eq!(r.name(), Some("Row"));
    assert!(q.definition("Missing").is_none());
}

#[test]
fn names_that_clash() {
    // Two different identities named `Row` are both kept.
    let other = Identity::new(Kind::object([("id", Kind::Integer)]).unwrap())
        .unwrap()
        .named("Row");
    let id =
        Identity::new(Kind::object([("a", Field::new(row())), ("b", Field::new(other))]).unwrap())
            .unwrap();
    let defs = id.definitions();
    assert_eq!(defs.len(), 2);
    assert!(defs.contains_key("Row") && defs.contains_key("Row_2"));
}

#[test]
fn names_are_in_the_json_form() {
    // A named identity's root is its own definition, so its name appears
    // in its JSON form, and comes back when it is read.
    let r = row();
    assert_eq!(r.name(), Some("Row"));
    let doc = to_json(&r);
    assert_eq!(doc["root"], json!({"ref": "Row"}));
    assert!(doc["definitions"]["Row"]["object"].is_object());
    let back = from_json(&doc).unwrap();
    assert_eq!(back.name(), Some("Row"));
    assert_eq!(back, r);
    // Renaming renames the definition and the references to it.
    let renamed = question().named("Query");
    assert_eq!(to_json(&renamed)["root"], json!({"ref": "Query"}));
    let parent = &to_json(&renamed)["definitions"]["Query"]["object"]["fields"]["parent"];
    assert_eq!(parent["kind"], json!({"union": ["null", {"ref": "Query"}]}));
    assert_ne!(renamed.quiddity(), question().quiddity());
}

#[test]
fn unnamed_identities_are_inlined() {
    let point = Identity::new(Kind::object([("x", Kind::Integer)]).unwrap()).unwrap();
    let id = Identity::new(Kind::list(point)).unwrap();
    assert!(id.definitions().is_empty());
    assert_eq!(
        to_json(&id)["root"],
        json!({"list": {"object": {"fields": {"x": {"kind": "integer"}}}}})
    );
}

#[test]
fn changing_fields() {
    let mut q = question();
    let before = q.clone();
    // Removing the only uses of `Row` removes its definition.
    q.remove_field_in("Question", "rows").unwrap();
    assert!(q.definitions().contains_key("Row"));
    q.remove_field_in("Question", "best").unwrap();
    assert!(!q.definitions().contains_key("Row"));
    // Adding one back brings it back.
    q.set_field_in("Question", "rows", Kind::list(row()))
        .unwrap();
    assert!(q.definitions().contains_key("Row"));
    assert_ne!(q, before);

    // A root object gains a recursive field: its root becomes its own
    // definition.
    let mut node = Identity::new(Kind::object([("value", Kind::Integer)]).unwrap())
        .unwrap()
        .named("Node");
    node.set_field("children", Kind::list(Kind::recursive()))
        .unwrap();
    assert_eq!(to_json(&node)["root"], json!({"ref": "Node"}));
    let tree = json!({"value": 1, "children": [{"value": 2, "children": []}]});
    node.digest(&tree).unwrap();
}

#[test]
fn failed_changes_change_nothing() {
    let mut q = question();
    let before = q.clone();
    // A union of two numbers is not valid; the identity is unchanged.
    let union = Kind::Union(vec![Kind::Integer, Kind::Number]);
    assert!(q.set_field_in("Question", "bad", union).is_err());
    // Two fields with one key.
    assert!(
        q.set_field_in(
            "Question",
            "other",
            Field::new(Kind::String).rename("brief")
        )
        .is_err()
    );
    assert!(q.set_field_in("Missing", "x", Kind::String).is_err());
    assert_eq!(q, before);
    assert!(q.set_domain("").is_err());
}

#[test]
fn constructors_check_what_they_build() {
    assert!(Kind::union([Kind::Integer, Kind::Number]).is_err());
    assert!(Kind::union(Vec::<Kind>::new()).is_err());
    assert!(Kind::tuple(Vec::<Kind>::new()).is_err());
    assert!(Kind::map(Kind::list(Kind::String), Kind::Any).is_err());
    assert!(Kind::object([("a", Kind::String), ("a", Kind::Integer)]).is_err());
    assert!(Kind::object_as_list([("a", Field::optional(Kind::String))]).is_err());
    assert!(Kind::internally_tagged("t", [("A", Some(Kind::String))]).is_err());
    assert!(Kind::adjacently_tagged("t", "t", [("A", None)]).is_err());
    let tagged = Kind::externally_tagged([("A", None), ("B", Some(Kind::Integer))]).unwrap();
    assert!(tagged.clone().by_discriminant([("A", 0)]).is_err());
    assert!(tagged.by_discriminant([("A", 0), ("B", 1)]).is_ok());
    // A recursive kind is checked once it is placed in an identity.
    assert!(Kind::union([Kind::Null, Kind::recursive()]).is_ok());
    assert!(Identity::new(Kind::recursive()).is_err());
}

/// With the `serde` feature, an identity is written and read in any serde
/// format, and reading checks it.
#[cfg(feature = "serde")]
#[test]
fn serde_round_trip() {
    let q = question();
    let text = serde_json::to_string(&q).unwrap();
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&text).unwrap(),
        to_json(&q)
    );
    let back: Identity = serde_json::from_str(&text).unwrap();
    assert_eq!(back, q);
    assert_eq!(back.quiddity(), q.quiddity());

    // Inside another serde type.
    #[derive(serde::Serialize, serde::Deserialize)]
    struct Config {
        version: u32,
        identity: Identity,
    }
    let config: Config = serde_json::from_value(json!({
        "version": 3,
        "identity": {"root": {"union": ["null", "string"]}}
    }))
    .unwrap();
    assert_eq!(
        config.identity,
        Identity::new(Kind::optional(Kind::String)).unwrap()
    );
    assert_eq!(
        serde_json::to_value(&config).unwrap()["identity"],
        to_json(&config.identity)
    );

    // Not valid: an error, naming the problem.
    let err = serde_json::from_value::<Identity>(json!({
        "root": {"union": ["integer", "number"]}
    }))
    .unwrap_err();
    assert!(err.to_string().contains("natively"), "{err}");
    let err = serde_json::from_str::<Identity>(r#"{"root": "text"}"#).unwrap_err();
    // A structural error is serde's, with its position in the text.
    assert!(err.to_string().contains("unknown variant `text`"), "{err}");
    assert_eq!((err.line(), err.column()), (1, 15));
}

/// Any `Serialize` value can be digested. A value has the digest of its
/// `serde_json::Value` form.
#[test]
fn serialize_values() {
    use std::collections::{BTreeMap, HashMap};

    #[derive(serde::Serialize)]
    enum Shape {
        Empty,
        Circle(f64),
        Rect { w: u32, h: u32 },
        Pair(u8, u8),
    }

    /// Bytes, written with `serialize_bytes`.
    struct Raw(&'static [u8]);
    impl serde::Serialize for Raw {
        fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
            s.serialize_bytes(self.0)
        }
    }

    #[derive(serde::Serialize)]
    struct Book {
        title: String,
        subtitle: Option<String>,
        shapes: Vec<Shape>,
        weights: HashMap<u32, f32>,
        flags: BTreeMap<bool, char>,
        raw: Raw,
        unit: (),
    }
    let book = Book {
        title: "Moby Dick".into(),
        subtitle: None,
        shapes: vec![
            Shape::Empty,
            Shape::Circle(1.5),
            Shape::Rect { w: 1, h: 2 },
            Shape::Pair(3, 4),
        ],
        // Exact in `f32`: with serde_json's `arbitrary_precision`, a `Value`
        // holds an `f32`'s shortest text, not its exact value.
        weights: HashMap::from([(1, 0.5), (20, 2.5)]),
        flags: BTreeMap::from([(true, 'y'), (false, 'n')]),
        raw: Raw(b"ab"),
        unit: (),
    };
    let any = Identity::new(Kind::Any).unwrap();
    let json = serde_json::to_value(&book).unwrap();
    assert_eq!(any.digest(&book).unwrap(), any.digest(&json).unwrap());
    #[cfg(feature = "serde_json")]
    assert_eq!(any.digest(&book).unwrap(), json.quiddity());

    let id = Identity::new(
        Kind::object([
            ("title", Field::new(Kind::String)),
            (
                "weights",
                Field::new(Kind::map(Kind::Integer, Kind::Number).unwrap()),
            ),
            ("raw", Field::new(Kind::bytes())),
        ])
        .unwrap(),
    )
    .unwrap();
    assert_eq!(id.digest(&book).unwrap(), id.digest(&json).unwrap());
}

/// Where a `serde_json::Value` cannot hold a value, it is kept: integers
/// beyond 64 bits, and non-finite floats.
#[test]
fn beyond_json() {
    let any = Identity::new(Kind::Any).unwrap();
    let integer = Identity::new(Kind::Integer).unwrap();
    let number = Identity::new(Kind::Number).unwrap();
    for n in [1i128 << 100, -(1i128 << 100), i128::MIN, i128::MAX] {
        assert_eq!(any.digest(&n).unwrap(), n.quiddity());
        assert_eq!(integer.digest(&n).unwrap(), n.quiddity());
    }
    assert_eq!(any.digest(&u128::MAX).unwrap(), u128::MAX.quiddity());
    for x in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert_eq!(any.digest(&x).unwrap(), x.quiddity());
        assert_eq!(number.digest(&x).unwrap(), x.quiddity());
    }
    // `f32` is widened exactly.
    assert_eq!(number.digest(&0.1f32).unwrap(), 0.1f32.quiddity());
    assert_ne!(number.digest(&0.1f32).unwrap(), 0.1f64.quiddity());
}

/// A value that cannot be serialized, or that writes a key twice, is an
/// error at the value itself.
#[test]
fn serialize_errors() {
    struct Failing;
    impl serde::Serialize for Failing {
        fn serialize<S: serde::Serializer>(&self, _: S) -> Result<S::Ok, S::Error> {
            Err(serde::ser::Error::custom("no"))
        }
    }
    struct Twice;
    impl serde::Serialize for Twice {
        fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
            use serde::ser::SerializeMap;
            let mut m = s.serialize_map(None)?;
            m.serialize_entry("a", &1)?;
            m.serialize_entry("a", &2)?;
            m.end()
        }
    }
    let any = Identity::new(Kind::Any).unwrap();
    let err = any.digest(&Failing).unwrap_err();
    assert_eq!(err.path, "$");
    assert!(err.message.contains("no"), "{err}");
    let err = any.digest(&Twice).unwrap_err();
    assert!(err.message.contains("twice"), "{err}");
    // A key that is not a string, a boolean or an integer.
    let pairs = std::collections::BTreeMap::from([((1, 2), 3)]);
    assert!(any.digest(&pairs).is_err());
    // Float keys, which serde_json writes as text, are not accepted.
    struct FloatKey;
    impl serde::Serialize for FloatKey {
        fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
            use serde::ser::SerializeMap;
            let mut m = s.serialize_map(Some(1))?;
            m.serialize_entry(&1.5f64, &1)?;
            m.end()
        }
    }
    let err = any.digest(&FloatKey).unwrap_err();
    assert!(err.message.contains("map key"), "{err}");
}
