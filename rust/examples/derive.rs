//! Digests of Rust values, with `#[derive(Quiddity)]`.
//!
//! ```sh
//! cargo run --example derive
//! ```

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use quiddity::{Digest, Digest64, IdentityMarker, Quiddity, QuiddityExt};

/// A second identity: what the task asks, regardless of how it is run.
#[derive(IdentityMarker)]
pub struct Meaning;

#[derive(Quiddity)]
#[quiddity(identities(Meaning), domain = "example.task")]
pub struct Task {
    prompt: String,
    documents: Vec<DocumentRef>,

    /// Not part of `Meaning`.
    #[quiddity(skip(Meaning))]
    priority: u8,

    /// The order of the tags does not count.
    #[quiddity(encode_as = set)]
    tags: Vec<String>,

    /// Left out when `None`, so that tasks from before this field existed
    /// keep their digests.
    #[quiddity(omit_none)]
    deadline: Option<SystemTime>,

    /// Rounded down to the cent.
    #[quiddity(decimals = 2)]
    budget: f64,

    /// Not part of any identity.
    #[quiddity(skip)]
    submitted_at: SystemTime,
}

#[derive(Quiddity)]
pub struct DocumentRef {
    label: String,
    content: Digest, // a digest is itself hashable
}

fn main() {
    let task = Task {
        prompt: "Summarize the report".into(),
        documents: vec![DocumentRef {
            label: "report".into(),
            content: "The report's text".quiddity(),
        }],
        priority: 1,
        tags: vec!["finance".into(), "q3".into()],
        deadline: None,
        budget: 12.50,
        submitted_at: SystemTime::now(),
    };
    let digest: Digest = task.quiddity();
    let meaning: Digest = task.quiddity_as::<Meaning>();
    println!("primary:  {digest}");
    println!("meaning:  {meaning}");

    // Fields that do not count, or that differ by less than a cent or in
    // order only, leave the digest unchanged.
    let same = Task {
        tags: vec!["q3".into(), "finance".into()],
        budget: 12.501,
        submitted_at: UNIX_EPOCH,
        ..task
    };
    assert_eq!(same.quiddity(), digest);

    // A change in priority changes the primary digest, and not `Meaning`'s.
    let urgent = Task {
        priority: 9,
        ..same
    };
    assert_ne!(urgent.quiddity(), digest);
    assert_eq!(urgent.quiddity_as::<Meaning>(), meaning);
    println!("urgent:   {}", urgent.quiddity());

    // A shorter form, for example for an ETag.
    let short: Digest64 = digest.micro();
    println!("micro:    {short}");

    // Digests are text, and parse back.
    let parsed: Digest = digest.to_string().parse().unwrap();
    assert_eq!(parsed, digest);

    // Values of different types with the same content have the same digest:
    // a struct and a map with its field names; a list and a tuple of the
    // same numbers; a duration and its number of nanoseconds.
    #[derive(Quiddity)]
    struct Interval {
        start: u64,
        end: u64,
    }
    let map = std::collections::BTreeMap::from([("start", 10u64), ("end", 15)]);
    assert_eq!(Interval { start: 10, end: 15 }.quiddity(), map.quiddity());
    assert_eq!(vec![1u8, 2].quiddity(), (1u32, 2i64).quiddity());
    assert_eq!(
        Duration::from_secs(1).quiddity(),
        1_000_000_000u64.quiddity()
    );
}
