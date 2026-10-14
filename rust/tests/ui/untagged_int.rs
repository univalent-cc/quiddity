use quiddity::Quiddity;

#[derive(Quiddity)]
#[quiddity(untagged, encode_as = int)]
enum Level {
    Low,
    High(u8),
}

fn main() {}
