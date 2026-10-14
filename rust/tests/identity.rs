//! Dynamic identities (spec 5): the README's example, and composing and
//! changing identities in code.

#![cfg(feature = "identity")]

use quiddity::QuiddityExt;
use quiddity::identity::{Field, Identity, Kind};
use serde_json::json;

#[test]
fn readme_example() {
    let identity = Identity::from_json(&json!({
        "quiddity": 1,
        "root": {"object": {"fields": {
            "prompt": {"kind": "string"},
            "priority": {"kind": "integer"},
            "asked_at": {"kind": {"instant": {}}}
        }}}
    }))
    .unwrap();
    let a = identity
        .digest(&json!({
            "prompt": "p", "priority": 1, "asked_at": "2024-01-01T00:00:00Z", "tag": "x"
        }))
        .unwrap();
    let b = identity
        .digest(&json!({
            "prompt": "p", "priority": "1", "asked_at": "2024-01-01T01:00:00+01:00"
        }))
        .unwrap();
    assert_eq!(a, b);
    let err = identity
        .digest(&json!({"prompt": 5, "priority": 1, "asked_at": "2024-01-01T00:00:00Z"}))
        .unwrap_err();
    assert_eq!(err.path, "$.prompt");
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
    let doc = q.to_json();
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
    // The document loads back to the same identity, with the same digest.
    let loaded = Identity::from_json(&doc).unwrap();
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
fn names_are_in_the_document() {
    // A named identity's root is its own definition, so its name appears
    // in its document, and comes back when the document is read.
    let r = row();
    assert_eq!(r.name(), Some("Row"));
    let doc = r.to_json();
    assert_eq!(doc["root"], json!({"ref": "Row"}));
    assert!(doc["definitions"]["Row"]["object"].is_object());
    let back = Identity::from_json(&doc).unwrap();
    assert_eq!(back.name(), Some("Row"));
    assert_eq!(back, r);
    // Renaming renames the definition and the references to it.
    let renamed = question().named("Query");
    assert_eq!(renamed.to_json()["root"], json!({"ref": "Query"}));
    let parent = &renamed.to_json()["definitions"]["Query"]["object"]["fields"]["parent"];
    assert_eq!(parent["kind"], json!({"union": ["null", {"ref": "Query"}]}));
    assert_ne!(renamed.quiddity(), question().quiddity());
}

#[test]
fn unnamed_identities_are_inlined() {
    let point = Identity::new(Kind::object([("x", Kind::Integer)]).unwrap()).unwrap();
    let id = Identity::new(Kind::list(point)).unwrap();
    assert!(id.definitions().is_empty());
    assert_eq!(
        id.to_json()["root"],
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
    assert_eq!(node.to_json()["root"], json!({"ref": "Node"}));
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

/// With the `serde` feature, an identity is written and read as its
/// document, and reading checks it.
#[cfg(feature = "serde")]
#[test]
fn serde_round_trip() {
    let q = question();
    let text = serde_json::to_string(&q).unwrap();
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&text).unwrap(),
        q.to_json()
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
        "identity": {"quiddity": 1, "root": {"union": ["null", "string"]}}
    }))
    .unwrap();
    assert_eq!(
        config.identity,
        Identity::new(Kind::optional(Kind::String)).unwrap()
    );
    assert_eq!(
        serde_json::to_value(&config).unwrap()["identity"],
        config.identity.to_json()
    );

    // Not valid: an error, naming the problem.
    let err = serde_json::from_value::<Identity>(json!({
        "quiddity": 1, "root": {"union": ["integer", "number"]}
    }))
    .unwrap_err();
    assert!(err.to_string().contains("natively"), "{err}");
    let err = serde_json::from_str::<Identity>(r#"{"quiddity": 1, "root": "text"}"#).unwrap_err();
    assert!(err.to_string().contains("unknown kind"), "{err}");
}
