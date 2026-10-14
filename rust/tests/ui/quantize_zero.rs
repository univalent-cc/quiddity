use quiddity::Quiddity;

#[derive(Quiddity)]
struct Count {
    #[quiddity(quantize = 0)]
    n: u32,
}

fn main() {}
