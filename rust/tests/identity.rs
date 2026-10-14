//! Dynamic identities (spec 5): the README's example, and building
//! identities in code.

#![cfg(feature = "identity")]

use quiddity::QuiddityExt;
use quiddity::identity::{Field, Identity, Kind, Omit, TimeOptions};
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
    // The wrong shape is an error, which names where.
    let err = identity
        .digest(&json!({"prompt": 5, "priority": 1, "asked_at": "2024-01-01T00:00:00Z"}))
        .unwrap_err();
    assert_eq!(err.path, "$.prompt");
}

#[test]
fn built_in_code() {
    let built = Identity::new(Kind::object([
        ("prompt", Field::new(Kind::String)),
        ("priority", Field::new(Kind::Integer)),
        (
            "asked_at",
            Field::new(Kind::Instant(TimeOptions::default())),
        ),
        (
            "tags",
            Field::new(Kind::optional(Kind::set(Kind::String)))
                .omit(Omit::None)
                .omit(Omit::Empty),
        ),
    ]))
    .unwrap();
    let loaded = Identity::from_json(&json!({
        "quiddity": 1,
        "root": {"object": {"fields": {
            "asked_at": {"kind": {"instant": {}}},
            "priority": {"kind": "integer"},
            "prompt": {"kind": "string"},
            "tags": {"kind": {"union": ["null", {"set": "string"}]}, "omit": ["empty", "none"]}
        }}}
    }))
    .unwrap();
    assert_eq!(built, loaded);
    assert_eq!(built.quiddity(), loaded.quiddity());
    assert_eq!(Identity::from_json(&built.to_json()).unwrap(), built);
}

#[test]
fn invalid_identities_are_rejected() {
    let err = Identity::new(Kind::Union(vec![Kind::Integer, Kind::Number])).unwrap_err();
    assert!(err.message.contains("natively"), "{err}");
    let err = Identity::new(Kind::reference("Row")).unwrap_err();
    assert!(err.message.contains("undefined"), "{err}");
}
