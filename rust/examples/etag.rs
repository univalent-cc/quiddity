//! Optimistic concurrency with `ETag` and `If-Match`: a change is accepted
//! only from a client that saw the current version.
//!
//! ```sh
//! cargo run --example etag
//! ```

use http::{HeaderMap, HeaderValue, StatusCode, header::IF_MATCH};
use quiddity::{Digest64, Quiddity, QuiddityExt};

#[derive(Clone, Quiddity)]
struct Task {
    prompt: String,
    priority: u8,
    /// Changes on every read; not part of the version.
    #[quiddity(skip)]
    reads: u32,
}

/// The `ETag` of a task: its digest, shortened, in quotes.
fn etag(task: &Task) -> HeaderValue {
    format!("\"{}\"", task.quiddity().micro()).parse().unwrap()
}

/// Applies `change` if the request's `If-Match` names the current version.
fn update(
    task: &mut Task,
    headers: &HeaderMap,
    change: impl FnOnce(&mut Task),
) -> Result<HeaderValue, StatusCode> {
    let claimed: Option<Digest64> = headers
        .get(IF_MATCH)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.trim_matches('"').parse().ok());
    match claimed {
        Some(claimed) if claimed == task.quiddity().micro() => {
            change(task);
            Ok(etag(task))
        }
        _ => Err(StatusCode::PRECONDITION_FAILED),
    }
}

fn main() {
    let mut task = Task {
        prompt: "Summarize the report".into(),
        priority: 1,
        reads: 0,
    };

    // Two clients read the task, and get the same ETag.
    let seen = etag(&task);
    task.reads += 2; // not part of the version: the ETag stays valid
    println!("ETag:        {}", seen.to_str().unwrap());

    let mut headers = HeaderMap::new();
    headers.insert(IF_MATCH, seen);

    // The first client's change is accepted, and gets a new ETag.
    let new = update(&mut task, &headers, |t| t.priority = 2).unwrap();
    println!("first:       accepted, new ETag {}", new.to_str().unwrap());

    // The second client's change is based on the old version: refused.
    let refused = update(&mut task, &headers, |t| t.prompt = "List the risks".into());
    assert_eq!(refused, Err(StatusCode::PRECONDITION_FAILED));
    println!("second:      {}", refused.unwrap_err());
    assert_eq!(task.prompt, "Summarize the report");
}
