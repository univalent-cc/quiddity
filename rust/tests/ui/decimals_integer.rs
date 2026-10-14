use quiddity::Quiddity;

#[derive(Quiddity)]
struct Count {
    #[quiddity(decimals = 2)]
    n: u32,
}

fn main() {}
