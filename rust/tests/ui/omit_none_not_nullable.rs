use quiddity::Quiddity;

#[derive(Quiddity)]
struct S {
    #[quiddity(omit_none)]
    count: u32,
}

fn main() {}
