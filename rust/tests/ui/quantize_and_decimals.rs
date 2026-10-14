use quiddity::Quiddity;

#[derive(Quiddity)]
struct Price {
    #[quiddity(quantize = 10, decimals = 2)]
    p: f64,
}

fn main() {}
