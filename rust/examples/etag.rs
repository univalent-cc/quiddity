//! Optimistic concurrency with `ETag` and `If-Match`: a change is accepted
//! only from a client that saw the current version.
//!
//! ```sh
//! cargo run --example etag
//! ```

use http::{HeaderMap, HeaderValue, StatusCode, header::IF_MATCH};
use quiddity::{Digest64, Quiddity, QuiddityExt};

#[derive(Clone, Quiddity)]
struct Book {
    title: String,
    pages: u16,
    /// Changes on every read; not part of the version.
    #[quiddity(skip)]
    reads: u32,
}

/// The `ETag` of a book: its digest, shortened, in quotes.
fn etag(book: &Book) -> HeaderValue {
    format!("\"{}\"", book.quiddity().micro()).parse().unwrap()
}

/// Applies `change` if the request's `If-Match` names the current version.
fn update(
    book: &mut Book,
    headers: &HeaderMap,
    change: impl FnOnce(&mut Book),
) -> Result<HeaderValue, StatusCode> {
    let claimed: Option<Digest64> = headers
        .get(IF_MATCH)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.trim_matches('"').parse().ok());
    match claimed {
        Some(claimed) if claimed == book.quiddity().micro() => {
            change(book);
            Ok(etag(book))
        }
        _ => Err(StatusCode::PRECONDITION_FAILED),
    }
}

fn main() {
    let mut book = Book {
        title: "Moby Dick".into(),
        pages: 635,
        reads: 0,
    };

    // Two clients read the book, and get the same ETag.
    let seen = etag(&book);
    book.reads += 2; // not part of the version: the ETag stays valid
    println!("ETag:        {}", seen.to_str().unwrap());

    let mut headers = HeaderMap::new();
    headers.insert(IF_MATCH, seen);

    // The first client's change is accepted, and gets a new ETag.
    let new = update(&mut book, &headers, |b| b.pages = 636).unwrap();
    println!("first:       accepted, new ETag {}", new.to_str().unwrap());

    // The second client's change is based on the old version: refused.
    let refused = update(&mut book, &headers, |b| b.title = "Typee".into());
    assert_eq!(refused, Err(StatusCode::PRECONDITION_FAILED));
    println!("second:      {}", refused.unwrap_err());
    assert_eq!(book.title, "Moby Dick");
}
