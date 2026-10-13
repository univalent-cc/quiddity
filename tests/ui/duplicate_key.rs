use quiddity::Quiddity;

#[derive(Quiddity)]
struct S {
    a: u8,
    #[quiddity(rename = "a")]
    b: u8,
}

fn main() {}
