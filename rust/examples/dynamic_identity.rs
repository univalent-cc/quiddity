//! Building identities at runtime, and digesting JSON values under them.
//!
//! An identity says which members of a value count, and how each is read.
//! Here one is built in code, applied to values, changed, written out as an
//! identity document, and read back.
//!
//! ```sh
//! cargo run --example dynamic_identity
//! ```

use quiddity::QuiddityExt;
use quiddity::identity::{Field, Identity, Kind};
use serde_json::json;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // --- Building -----------------------------------------------------------

    // An identity with a name becomes a definition of the identities it is
    // nested in, under that name.
    let document_ref = Identity::new(Kind::object([
        ("label", Kind::String),
        ("content", Kind::String), // a digest, in its text form
    ])?)?
    .named("DocumentRef");

    // Each variant of a tagged union is a unit variant (`None`) or an object.
    let answer_format = Kind::internally_tagged(
        "type",
        [
            ("Free", None),
            (
                "Paragraphs",
                Some(Kind::object([("count", Kind::Integer)])?),
            ),
        ],
    )?;

    let mut task = Identity::new(Kind::object([
        ("prompt", Field::new(Kind::String)),
        ("documents", Field::new(Kind::list(document_ref))),
        ("priority", Field::new(Kind::Integer)),
        ("format", Field::new(answer_format)),
        // Left out when null or missing.
        ("note", Field::optional(Kind::String)),
        // Left out when empty; the order of its members does not count.
        ("tags", Field::omit_empty(Kind::set(Kind::String))),
        // Read from the member `askedAt`, digested under the key `asked_at`
        // (as a value with the member `asked_at` would be), and rounded
        // down to the second.
        (
            "askedAt",
            Field::new(Kind::instant())
                .rename("asked_at")
                .quantize(1_000_000_000),
        ),
        // Another task, or null: `Kind::recursive()` is the identity being built.
        ("follows", Field::optional(Kind::recursive())),
    ])?)?
    .named("Task");
    // A domain keeps this identity's digests apart from those of any other
    // identity with the same fields.
    task.set_domain("example.task")?;

    // --- Digesting ----------------------------------------------------------

    let value = json!({
        "prompt": "Summarize the report",
        "documents": [{"label": "report", "content": "sha256:9f2c…"}],
        "priority": 1,
        "format": {"type": "Paragraphs", "count": 2},
        "tags": ["finance", "q3"],
        "askedAt": "2026-10-14T09:30:00.250Z",
        "submitted_by": "alice", // not a field: skipped
    });
    let digest = task.digest(&value)?;
    println!("digest:            {digest}");

    // The same task, written differently: members in another order, the
    // integer as text, the instant in another time zone and with other
    // milliseconds, the set reordered, and a null note.
    let same = json!({
        "askedAt": "2026-10-14T11:30:00+02:00",
        "tags": ["q3", "finance"],
        "note": null,
        "format": {"count": 2, "type": "Paragraphs"},
        "priority": "1",
        "documents": [{"content": "sha256:9f2c…", "label": "report"}],
        "prompt": "Summarize the report",
        "submitted_by": "bob",
    });
    assert_eq!(task.digest(&same)?, digest);
    println!("written differently: same digest");

    // A change to a field that counts changes the digest.
    let mut other = value.clone();
    other["priority"] = json!(2);
    assert_ne!(task.digest(&other)?, digest);
    println!("another priority:  {}", task.digest(&other)?);

    // A recursive value.
    let mut follow_up = value.clone();
    follow_up["prompt"] = json!("Now list the risks");
    follow_up["follows"] = value.clone();
    println!("follow-up:         {}", task.digest(&follow_up)?);

    // A value of the wrong shape is an error that says where.
    let mut wrong = value.clone();
    wrong["documents"][0]["label"] = json!(7);
    let err = task.digest(&wrong).unwrap_err();
    assert_eq!(err.path, "$.documents[0].label");
    println!("wrong shape:       {err}");

    // --- Changing -----------------------------------------------------------

    // A change that would make the identity invalid fails and changes
    // nothing: `prompt` is already a key.
    let before = task.clone();
    assert!(
        task.set_field_in("Task", "title", Field::new(Kind::String).rename("prompt"))
            .is_err()
    );
    assert_eq!(task, before);

    // Priority no longer counts. Fields of a named identity's root are
    // changed in its definition.
    task.remove_field_in("Task", "priority")?;
    assert_eq!(task.digest(&value)?, task.digest(&other)?);
    println!("without priority:  {}", task.digest(&value)?);

    // --- Documents ----------------------------------------------------------

    // An identity is data: written out as JSON, it can be read by any
    // implementation of quiddity, in any language.
    let document = task.to_json();
    println!("\nidentity document:\n{document:#}\n");

    let loaded = Identity::from_json(&document)?;
    assert_eq!(loaded, task);
    assert_eq!(loaded.digest(&value)?, task.digest(&value)?);

    // An identity has a digest of its own, which every implementation
    // computes the same way. It tells whether two identities are the same.
    println!("identity's digest: {}", task.quiddity());

    // A document that is not valid is an error that says where.
    let err = Identity::from_json(&json!({
        "quiddity": 1,
        "root": {"union": ["integer", "number"]}
    }))
    .unwrap_err();
    println!("invalid document:  {err}");

    Ok(())
}
