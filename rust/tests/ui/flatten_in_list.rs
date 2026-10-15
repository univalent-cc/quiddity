use quiddity::Quiddity;

#[derive(Quiddity)]
struct Page {
    number: u32,
}

#[derive(Quiddity)]
#[quiddity(encode_as = list)]
struct Query {
    term: String,
    #[quiddity(flatten)]
    page: Page,
}

fn main() {}
