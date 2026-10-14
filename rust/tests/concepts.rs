//! The examples of the repository's README (its "Concepts" section), and
//! of this crate's README's "Building identities in code", kept here so
//! that they are compiled and checked.

#![cfg(feature = "identity")]

use quiddity::identity::{Field, Identity, Kind};
use serde_json::json;

type Result = std::result::Result<(), Box<dyn std::error::Error>>;

#[test]
fn example() -> Result {
    let document_ref = Identity::new(Kind::object([
        ("label", Kind::String),
        ("content", Kind::String), // a digest, in its text form
    ])?)?
    .named("DocumentRef");

    let task = Identity::new(Kind::object([
        ("prompt", Field::new(Kind::String)),
        ("documents", Field::new(Kind::list(document_ref))),
        ("priority", Field::new(Kind::Integer)),
    ])?)?
    .named("Task");

    let digest = task.digest(&json!({
        "prompt": "Summarize the report",
        "documents": [{"label": "report", "content": "sha256:…"}],
        "priority": 1,
        "submitted_at": "2026-10-14T09:30:00Z", // not a field: not part of the digest
    }))?;
    let later = task.digest(&json!({
        "prompt": "Summarize the report",
        "documents": [{"label": "report", "content": "sha256:…"}],
        "priority": 1,
        "submitted_at": "2026-10-15T17:45:00Z",
    }))?;
    assert_eq!(digest, later);
    assert!(digest.to_string().starts_with("sha256:"));
    Ok(())
}

#[test]
fn additional_identities() -> Result {
    let primary = Identity::new(Kind::object([
        ("prompt", Kind::String),
        ("priority", Kind::Integer),
    ])?)?;
    let meaning = Identity::new(Kind::object([
        ("prompt", Kind::String),
        ("answer_format", Kind::String),
    ])?)?;

    let task = json!({
        "prompt": "Summarize the report",
        "priority": 1,
        "answer_format": "one paragraph",
        "submitted_at": "2026-10-14T09:30:00Z",
    });
    let a = primary.digest(&task)?; // the prompt and priority
    let b = meaning.digest(&task)?; // the prompt and answer format
    assert_ne!(a, b);
    Ok(())
}

#[test]
fn quantization() -> Result {
    let reading = Identity::new(Kind::object([
        ("price", Field::new(Kind::Number).decimals(2)),
        ("count", Field::new(Kind::Integer).quantize(100)),
        ("at", Field::new(Kind::instant()).quantize(1_000_000_000)), // to the second
    ])?)?;
    let a =
        reading.digest(&json!({"price": 12.989, "count": 149, "at": "2024-01-01T00:00:00.75Z"}))?;
    let b = reading.digest(&json!({"price": 12.98, "count": 100, "at": "2024-01-01T00:00:00Z"}))?;
    assert_eq!(a, b);
    let c = reading.digest(&json!({"price": 12.98, "count": 99, "at": "2024-01-01T00:00:00Z"}))?;
    assert_ne!(a, c);
    Ok(())
}

#[test]
fn domains() -> Result {
    let fields = Kind::object([("to", Kind::String), ("amount", Kind::Integer)])?;
    let mut payment = Identity::new(fields)?;
    let mut refund = payment.clone();
    payment.set_domain("example.payment")?;
    refund.set_domain("example.refund")?;

    let value = json!({"to": "bob", "amount": 100});
    assert_ne!(payment.digest(&value)?, refund.digest(&value)?);
    Ok(())
}

#[test]
fn how_values_are_digested() -> Result {
    let id = Identity::new(Kind::object([
        ("count", Kind::Integer),
        ("at", Kind::instant()),
        ("tags", Kind::set(Kind::String)),
    ])?)?;
    let a = id.digest(&json!({"count": 5, "at": "2024-01-01T00:00:00Z", "tags": ["x", "y"]}))?;
    let b =
        id.digest(&json!({"tags": ["y", "x"], "at": "2024-01-01T01:00:00+01:00", "count": "5.0"}))?;
    assert_eq!(a, b);
    Ok(())
}

#[test]
fn identity_documents() -> Result {
    let id = Identity::new(Kind::object([("prompt", Kind::String)])?)?;
    let document = id.to_json();
    assert_eq!(
        document,
        json!({"quiddity": 1, "root": {"object": {"fields": {"prompt": {"kind": "string"}}}}})
    );
    assert_eq!(Identity::from_json(&document)?, id);
    Ok(())
}

/// This crate's README, "Building identities in code".
#[test]
fn building_identities_in_code() -> Result {
    let row = Identity::new(Kind::object([
        ("label", Kind::String),
        ("value", Kind::Number),
    ])?)?
    .named("Row");

    let mut question = Identity::new(Kind::object([
        ("brief", Field::new(Kind::String)),
        ("note", Field::optional(Kind::String)), // null or missing: left out
        ("tags", Field::omit_empty(Kind::set(Kind::String))),
        ("rows", Field::new(Kind::list(row))),
        ("parent", Field::optional(Kind::recursive())), // a question
    ])?)?
    .named("Question");
    question.set_domain("example.question")?;
    question.set_field_in("Question", "asked_at", Kind::instant())?;

    let document = question.to_json();
    assert_eq!(document["root"], json!({"ref": "Question"}));
    assert_eq!(document["domain"], json!("example.question"));
    Ok(())
}
