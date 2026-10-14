use quiddity::Quiddity;

#[derive(Quiddity)]
#[quiddity(rename_all = "Train-Case")]
struct Request {
    user_id: u8,
}

fn main() {}
