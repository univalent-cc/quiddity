use quiddity::{IdentityMarker, Quiddity};

#[derive(IdentityMarker)]
struct Meaning;

#[derive(Quiddity)]
struct S {
    #[quiddity(skip(Meaning))]
    a: u8,
}

fn main() {}
