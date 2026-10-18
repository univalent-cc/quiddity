use quiddity::{IdentityMarker, Quiddity};
use serde::Serialize;

#[derive(IdentityMarker)]
pub struct Meaning;

#[derive(Serialize, Quiddity)]
#[quiddity(identities(Meaning), serde_attributes(Primary))]
struct Book {
    title: String,
    #[quiddity(skip)]
    pages: u16,
}

fn main() {}
