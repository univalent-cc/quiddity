use quiddity::Quiddity;
use serde::Serialize;

pub struct Other {
    pub n: u32,
}

#[derive(Serialize, Quiddity)]
#[serde(remote = "Other")]
#[quiddity(serde_attributes)]
struct OtherDef {
    n: u32,
}

fn main() {}
