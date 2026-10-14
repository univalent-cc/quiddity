use quiddity::Quiddity;

#[derive(Quiddity)]
struct Label {
    #[quiddity(quantize = 10)]
    text: String,
}

fn main() {}
