use quiddity::Quiddity;
use serde::Serialize;

fn upper<S: serde::Serializer>(t: &str, s: S) -> Result<S::Ok, S::Error> {
    s.serialize_str(&t.to_uppercase())
}

#[derive(Serialize, Quiddity)]
#[quiddity(serde_attributes)]
struct Item {
    #[serde(serialize_with = "upper")]
    name: String,
}

fn main() {}
