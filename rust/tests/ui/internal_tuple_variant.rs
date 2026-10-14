use quiddity::Quiddity;

#[derive(Quiddity)]
#[quiddity(tag = "type")]
enum Shape {
    Empty,
    Circle(f64),
}

fn main() {}
