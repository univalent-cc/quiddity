use quiddity::Quiddity;

#[derive(Quiddity)]
struct S {
    #[quiddity(encode_as = serde)]
    a: u8,
}

fn main() {}
