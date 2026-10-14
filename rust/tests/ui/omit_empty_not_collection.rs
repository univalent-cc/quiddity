use quiddity::Quiddity;

#[derive(Quiddity)]
struct Retries {
    #[quiddity(omit_empty)]
    count: u32,
}

fn main() {}
