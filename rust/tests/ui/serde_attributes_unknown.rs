use quiddity::Quiddity;
use serde::Serialize;

#[derive(Serialize, Quiddity)]
#[quiddity(serde_attributes)]
struct Task {
    #[serde(rename = "p", someday_maybe)]
    prompt: String,
}

fn main() {}
