use quiddity::Quiddity;

fn f(_: &u8, e: &mut quiddity::Encoder) {
    e.null();
}

#[derive(Quiddity)]
struct S {
    #[quiddity(with = f, encode_as = display)]
    a: u8,
}

fn main() {}
