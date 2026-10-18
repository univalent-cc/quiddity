//! Building identities at runtime, and digesting JSON values under them.
//!
//! An identity says which members of a value count, and how each is read.
//! Here one is built in code, applied to values, changed, written out as
//! JSON, and read back.
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
    let author = Identity::new(Kind::object([
        ("name", Kind::String),
        ("born", Kind::Integer),
    ])?)?
    .named("Author");

    // Each variant of a tagged union is a unit variant (`None`) or an object.
    let format = Kind::internally_tagged(
        "type",
        [
            ("Ebook", None),
            ("Print", Some(Kind::object([("volumes", Kind::Integer)])?)),
        ],
    )?;

    let mut book = Identity::new(Kind::object([
        ("title", Field::new(Kind::String)),
        ("authors", Field::new(Kind::list(author))),
        ("pages", Field::new(Kind::Integer)),
        ("format", Field::new(format)),
        // Left out when null or missing.
        ("subtitle", Field::optional(Kind::String)),
        // Left out when empty; the order of its members does not count.
        ("genres", Field::omit_empty(Kind::set(Kind::String))),
        // When the library record was made. Read from the member
        // `cataloguedAt`, digested under the key `catalogued_at` (as a value
        // with the member `catalogued_at` would be), and rounded down to the
        // second.
        (
            "cataloguedAt",
            Field::new(Kind::instant())
                .rename("catalogued_at")
                .quantize(1_000_000_000),
        ),
        // The book this one follows, or null: `Kind::recursive()` is the
        // identity being built.
        ("follows", Field::optional(Kind::recursive())),
    ])?)?
    .named("Book");
    // A domain keeps this identity's digests apart from those of any other
    // identity with the same fields.
    book.set_domain("example.book")?;

    // --- Digesting ----------------------------------------------------------

    let value = json!({
        "title": "Moby Dick",
        "authors": [{"name": "Herman Melville", "born": 1819}],
        "pages": 635,
        "format": {"type": "Print", "volumes": 3},
        "genres": ["novel", "adventure"],
        "cataloguedAt": "2026-10-14T09:30:00.250Z",
        "last_opened": "2026-10-16T21:05:00Z", // not a field: skipped
    });
    let digest = book.digest(&value)?;
    println!("digest:            {digest}");

    // The same book, written differently: members in another order, the
    // integer as text, the instant in another time zone and with other
    // milliseconds, the set reordered, and a null subtitle.
    let same = json!({
        "cataloguedAt": "2026-10-14T11:30:00+02:00",
        "genres": ["adventure", "novel"],
        "subtitle": null,
        "format": {"volumes": 3, "type": "Print"},
        "pages": "635",
        "authors": [{"born": 1819, "name": "Herman Melville"}],
        "title": "Moby Dick",
        "last_opened": "2026-10-16T21:15:00Z",
    });
    assert_eq!(book.digest(&same)?, digest);
    println!("written differently: same digest");

    // A change to a field that counts changes the digest.
    let mut other = value.clone();
    other["pages"] = json!(720);
    assert_ne!(book.digest(&other)?, digest);
    println!("other pages:       {}", book.digest(&other)?);

    // A recursive value.
    let mut typee = value.clone();
    typee["title"] = json!("Typee");
    typee["pages"] = json!(298);
    let mut sequel = value.clone();
    sequel["follows"] = typee;
    println!("follows Typee:     {}", book.digest(&sequel)?);

    // A value of the wrong shape is an error that says where.
    let mut wrong = value.clone();
    wrong["authors"][0]["name"] = json!(7);
    let err = book.digest(&wrong).unwrap_err();
    assert_eq!(err.path, "$.authors[0].name");
    println!("wrong shape:       {err}");

    // --- Changing -----------------------------------------------------------

    // A change that would make the identity invalid fails and changes
    // nothing: `title` is already a key.
    let before = book.clone();
    assert!(
        book.set_field_in("Book", "name", Field::new(Kind::String).rename("title"))
            .is_err()
    );
    assert_eq!(book, before);

    // Pages no longer count. Fields of a named identity's root are changed
    // in its definition.
    book.remove_field_in("Book", "pages")?;
    assert_eq!(book.digest(&value)?, book.digest(&other)?);
    println!("without pages:     {}", book.digest(&value)?);

    // --- As JSON ------------------------------------------------------------

    // An identity is data: written out as JSON, it can be read by any
    // implementation of quiddity, in any language.
    let json = book.to_json();
    println!("\nidentity as JSON:\n{json:#}\n");

    let loaded = Identity::from_json(&json)?;
    assert_eq!(loaded, book);
    assert_eq!(loaded.digest(&value)?, book.digest(&value)?);

    // An identity has a digest of its own, which every implementation
    // computes the same way. It tells whether two identities are the same.
    println!("identity's digest: {}", book.quiddity());

    // An identity that is not valid is an error that says where.
    let err = Identity::from_json(&json!({
        "root": {"union": ["integer", "number"]}
    }))
    .unwrap_err();
    println!("invalid identity:  {err}");

    Ok(())
}
