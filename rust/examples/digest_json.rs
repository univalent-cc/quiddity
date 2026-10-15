//! Digests JSON files under an identity read from a JSON file.
//!
//! ```sh
//! cargo run --example digest_json -- \
//!     examples/data/task.identity.json examples/data/task.json
//! ```
//!
//! With no value files, reads one JSON value from standard input.

use std::{env, fs, io, process};

use quiddity::QuiddityExt;
use quiddity::identity::Identity;
use serde_json::Value;

fn main() {
    if let Err(e) = run() {
        eprintln!("error: {e}");
        process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = env::args().skip(1);
    let Some(identity_path) = args.next() else {
        return Err("usage: digest_json <identity.json> [value.json …]".into());
    };
    let identity =
        Identity::from_json(&read(&identity_path)?).map_err(|e| format!("{identity_path}: {e}"))?;
    println!("{}  (identity)", identity.quiddity());

    let paths: Vec<String> = args.collect();
    if paths.is_empty() {
        let value: Value = serde_json::from_reader(io::stdin())?;
        println!("{}  -", identity.digest(&value)?);
    }
    for path in paths {
        let digest = identity
            .digest(&read(&path)?)
            .map_err(|e| format!("{path}: {e}"))?;
        println!("{digest}  {path}");
    }
    Ok(())
}

fn read(path: &str) -> Result<Value, Box<dyn std::error::Error>> {
    let text = fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?;
    Ok(serde_json::from_str(&text).map_err(|e| format!("{path}: {e}"))?)
}
