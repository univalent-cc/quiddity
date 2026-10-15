//! Exporting a Rust type's identity, so that another program, in any
//! language, can digest the type's JSON form and get the same digest.
//!
//! ```sh
//! cargo run --example export_identity
//! ```

use quiddity::{Quiddity, QuiddityExt};
use serde::Serialize;

// The serde and quiddity attributes name fields and variants the same way,
// so that the JSON form's keys are the identity's keys.
#[derive(Serialize, Quiddity)]
#[serde(rename_all = "camelCase")]
#[quiddity(rename_all = "camelCase", domain = "example.question")]
pub struct Question {
    brief: String,
    rows: Vec<Row>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[quiddity(omit_none)]
    note: Option<String>,
    #[quiddity(encode_as = set)]
    tags: Vec<String>,
    answer_format: Format,
    #[serde(skip)]
    #[quiddity(skip)]
    cache_hits: u32,
}

#[derive(Serialize, Quiddity)]
pub struct Row {
    label: String,
    #[quiddity(decimals = 2)]
    value: f64,
}

#[derive(Serialize, Quiddity)]
#[serde(tag = "type")]
#[quiddity(tag = "type")]
pub enum Format {
    Free,
    Paragraphs { count: u32 },
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let identity = <Question as Quiddity>::construct_identity()?;
    println!("identity as JSON:\n{:#}\n", identity.to_json());

    let question = Question {
        brief: "Which quarter grew most?".into(),
        rows: vec![
            Row {
                label: "q2".into(),
                value: 1.25,
            },
            Row {
                label: "q3".into(),
                value: 1.5,
            },
        ],
        note: None,
        tags: vec!["finance".into()],
        answer_format: Format::Paragraphs { count: 1 },
        cache_hits: 3,
    };

    // The value's digest, from the derive.
    let derived = question.quiddity();
    // The digest of its JSON form under the exported identity, as any other
    // implementation computes it.
    let json = serde_json::to_value(&question)?;
    println!("JSON form:\n{json:#}\n");
    let dynamic = identity.digest(&json)?;

    assert_eq!(derived, dynamic);
    println!("derived:  {derived}");
    println!("dynamic:  {dynamic}");
    Ok(())
}
