use quiddity::Quiddity;

#[derive(Quiddity)]
#[quiddity(encode_as = list)]
struct S {
    #[quiddity(omit_none)]
    a: Option<u8>,
}

fn main() {}
