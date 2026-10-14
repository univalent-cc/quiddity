use quiddity::Quiddity;

#[derive(Quiddity)]
#[quiddity(encode_as = list)]
enum E {
    A,
}

fn main() {}
