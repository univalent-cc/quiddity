use quiddity::Quiddity;

// As in a build without the crate's `serde` feature: the serde derive and
// its attributes are compiled out, and the mode would read nothing.
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "lowercase"))]
#[derive(Quiddity)]
#[quiddity(serde_attributes)]
enum Mode {
    None,
    Stated,
}

fn main() {}
