use quiddity::Quiddity;

#[derive(Quiddity)]
#[quiddity(encode_as = set)]
struct Tags {
    tags: Vec<String>,
}

fn main() {}
