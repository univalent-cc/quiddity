use quiddity::Quiddity;
use serde::Serialize;

#[derive(Serialize, Quiddity)]
#[serde(tag = "type")]
#[quiddity(serde_attributes)]
struct Clash {
    r#type: String,
}

fn main() {}
