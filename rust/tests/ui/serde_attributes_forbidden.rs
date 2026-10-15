use quiddity::{IdentityMarker, Quiddity};
use serde::Serialize;

#[derive(IdentityMarker)]
pub struct Meaning;

#[derive(Serialize, Quiddity)]
#[quiddity(identities(Meaning), serde_attributes(Primary))]
struct Task {
    prompt: String,
    #[quiddity(skip)]
    priority: u8,
}

fn main() {}
