use quiddity::Quiddity;

#[derive(Quiddity)]
#[quiddity(untagged)]
enum Point {
    Int { x: i64, y: i64 },
    Float { x: f64, y: f64 },
}

fn main() {}
