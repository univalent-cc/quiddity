use quiddity::Quiddity;

#[derive(Quiddity)]
struct Page {
    number: u32,
}

#[derive(Quiddity)]
struct Query {
    #[quiddity(flatten, rename = "p")]
    page: Page,
}

fn main() {}
