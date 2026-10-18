use quiddity::Quiddity;
use serde::Serialize;

#[derive(Serialize, Quiddity)]
#[quiddity(serde_attributes)]
struct Book {
    #[serde(rename = "t", someday_maybe)]
    title: String,
}

fn main() {}
