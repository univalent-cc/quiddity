use quiddity::Quiddity;

#[derive(Quiddity)]
#[quiddity(encode_as = list)]
struct S {
    #[quiddity(none_as_missing)]
    a: Option<u8>,
}

fn main() {}
