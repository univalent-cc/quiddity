use quiddity::Quiddity;

#[derive(Quiddity)]
#[quiddity(untagged)]
enum Status {
    Unknown,
    Missing,
    Code(u16),
}

fn main() {}
