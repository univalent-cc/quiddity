use quiddity::Quiddity;

#[derive(Quiddity)]
struct Page {
    number: u32,
}

#[derive(Quiddity)]
struct Query(String, #[quiddity(flatten)] Page);

fn main() {}
