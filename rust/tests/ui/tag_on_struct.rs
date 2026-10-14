use quiddity::Quiddity;

#[derive(Quiddity)]
#[quiddity(tag = "type")]
struct Point {
    x: i32,
}

fn main() {}
