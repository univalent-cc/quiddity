mod a {
    #[derive(quiddity::IdentityMarker)]
    pub struct Meaning;
}

mod b {
    #[derive(quiddity::IdentityMarker)]
    pub struct Meaning;
}

#[derive(quiddity::Quiddity)]
#[quiddity(identities(a::Meaning, b::Meaning))]
struct S {
    x: u8,
}

fn main() {}
