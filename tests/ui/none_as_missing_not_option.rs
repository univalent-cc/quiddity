use quiddity::Quiddity;

#[derive(Quiddity)]
struct S {
    #[quiddity(none_as_missing)]
    count: u32,
}

fn main() {}
