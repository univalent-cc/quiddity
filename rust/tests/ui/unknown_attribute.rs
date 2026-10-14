use quiddity::Quiddity;

#[derive(Quiddity)]
struct S {
    #[quiddity(skip_if_empty)]
    a: Vec<u8>,
}

fn main() {}
