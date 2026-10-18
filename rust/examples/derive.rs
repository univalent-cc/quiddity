//! Digests of Rust values, with `#[derive(Quiddity)]`.
//!
//! ```sh
//! cargo run --example derive
//! ```

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use quiddity::{Digest, Digest64, IdentityMarker, Quiddity, QuiddityExt};

/// A second identity: the work, whatever the edition.
#[derive(IdentityMarker)]
pub struct Meaning;

#[derive(Quiddity)]
#[quiddity(identities(Meaning), domain = "example.book")]
pub struct Book {
    title: String,
    authors: Vec<Author>,
    text: Digest, // a digest is itself hashable

    /// Not part of `Meaning`.
    #[quiddity(skip(Meaning))]
    pages: u16,

    /// The order of the genres does not count.
    #[quiddity(encode_as = set)]
    genres: Vec<String>,

    /// Left out when `None`, so that books from before this field existed
    /// keep their digests.
    #[quiddity(omit_none)]
    subtitle: Option<String>,

    /// Rounded down to the cent.
    #[quiddity(decimals = 2)]
    price: f64,

    /// When you last read it: not part of any identity.
    #[quiddity(skip)]
    last_opened: SystemTime,
}

#[derive(Quiddity)]
pub struct Author {
    name: String,
    born: u16,
}

fn main() {
    let book = Book {
        title: "Moby Dick".into(),
        authors: vec![Author {
            name: "Herman Melville".into(),
            born: 1819,
        }],
        text: "Call me Ishmael. Some years ago…".quiddity(),
        pages: 635,
        genres: vec!["novel".into(), "adventure".into()],
        subtitle: Some("or, The Whale".into()),
        price: 12.50,
        last_opened: SystemTime::now(),
    };
    let digest: Digest = book.quiddity();
    let meaning: Digest = book.quiddity_as::<Meaning>();
    println!("primary:  {digest}");
    println!("meaning:  {meaning}");

    // Fields that do not count, or that differ by less than a cent or in
    // order only, leave the digest unchanged.
    let same = Book {
        genres: vec!["adventure".into(), "novel".into()],
        price: 12.501,
        last_opened: UNIX_EPOCH,
        ..book
    };
    assert_eq!(same.quiddity(), digest);

    // Another edition changes the primary digest, and not `Meaning`'s.
    let reprint = Book { pages: 720, ..same };
    assert_ne!(reprint.quiddity(), digest);
    assert_eq!(reprint.quiddity_as::<Meaning>(), meaning);
    println!("reprint:  {}", reprint.quiddity());

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
