use quiddity::Quiddity;

#[derive(Quiddity)]
#[quiddity(transparent)]
struct S {
    a: u8,
    b: u8,
}

fn main() {}
