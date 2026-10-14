//! Checks the encoder and digests against `test-vectors/quiddity-v1.json`,
//! at the root of the repository, outside this crate.
//!
//! The vectors are not part of the published crate. In a published crate
//! (recognized by the `.cargo_vcs_info.json` that `cargo package` adds),
//! these tests skip; anywhere else, a missing vectors file is a failure.

use std::string::{String, ToString};
use std::vec::Vec;
use std::{format, vec};

use serde_json::Value as Json;

use crate::{Digest, Digest64, Digest128, Encoder, Quiddity, QuiddityExt};

use std::path::Path;

/// A data-model value, parsed from the vectors' typed notation.
#[derive(Debug)]
enum Value {
    Null,
    Bool(bool),
    /// Two's-complement big-endian bytes, any length.
    Int(Vec<u8>),
    Float(f64),
    String(String),
    /// Text that may contain surrogates.
    CodePoints(Vec<u32>),
    Bytes(Vec<u8>),
    List(Vec<Value>),
    Set(Vec<Value>),
    Map(Vec<(Value, Value)>),
}

impl Quiddity for Value {
    fn encode(&self, e: &mut Encoder) {
        match self {
            Value::Null => e.null(),
            Value::Bool(b) => e.bool(*b),
            Value::Int(bytes) => e.int_twos_complement(bytes),
            Value::Float(x) => e.float(*x),
            Value::String(s) => e.str(s),
            Value::CodePoints(cps) => e.utf16(cps.iter().flat_map(|&cp| utf16_units(cp))),
            Value::Bytes(b) => e.bytes(b),
            Value::List(items) => {
                let mut l = e.list();
                for item in items {
                    l.item(item);
                }
            }
            Value::Set(members) => {
                let mut s = e.set();
                for member in members {
                    s.member(member);
                }
            }
            Value::Map(entries) => {
                let mut m = e.map();
                for (k, v) in entries {
                    m.entry(k, v);
                }
            }
        }
    }
}

fn parse(json: &Json) -> Value {
    let obj = json.as_object().expect("a value is an object");
    assert_eq!(obj.len(), 1, "a value has one key: {json}");
    let (kind, v) = obj.iter().next().unwrap();
    match kind.as_str() {
        "null" => Value::Null,
        "bool" => Value::Bool(v.as_bool().unwrap()),
        "int" => Value::Int(decimal_to_twos_complement(v.as_str().unwrap())),
        "float" => {
            let bits = u64::from_str_radix(v.as_str().unwrap(), 16).unwrap();
            Value::Float(f64::from_bits(bits))
        }
        "string" => Value::String(v.as_str().unwrap().to_string()),
        "code_points" => Value::CodePoints(
            v.as_array()
                .unwrap()
                .iter()
                .map(|cp| cp.as_u64().unwrap() as u32)
                .collect(),
        ),
        "bytes" => Value::Bytes(from_hex(v.as_str().unwrap())),
        "list" => Value::List(v.as_array().unwrap().iter().map(parse).collect()),
        "set" => Value::Set(v.as_array().unwrap().iter().map(parse).collect()),
        "map" => Value::Map(
            v.as_array()
                .unwrap()
                .iter()
                .map(|pair| (parse(&pair[0]), parse(&pair[1])))
                .collect(),
        ),
        other => panic!("unknown kind {other:?}"),
    }
}

/// A code point as UTF-16 code units: a surrogate as itself, so that
/// surrogates given separately can still form a pair.
fn utf16_units(cp: u32) -> Vec<u16> {
    match char::from_u32(cp) {
        Some(c) => c.encode_utf16(&mut [0; 2]).to_vec(),
        None => vec![cp as u16],
    }
}

/// A decimal integer of any size as two's-complement big-endian bytes.
fn decimal_to_twos_complement(s: &str) -> Vec<u8> {
    let (negative, digits) = match s.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, s),
    };
    // Magnitude, big-endian, with a leading zero byte for the sign.
    let mut bytes = vec![0u8];
    for d in digits.bytes() {
        let mut carry = u32::from(d - b'0');
        for b in bytes.iter_mut().rev() {
            let x = u32::from(*b) * 10 + carry;
            *b = x as u8;
            carry = x >> 8;
        }
        while carry > 0 {
            bytes.insert(0, carry as u8);
            carry >>= 8;
        }
        if bytes[0] & 0x80 != 0 {
            bytes.insert(0, 0);
        }
    }
    if negative {
        crate::number::negate(&mut bytes);
    }
    bytes
}

fn from_hex(s: &str) -> Vec<u8> {
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
        .collect()
}

fn to_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn encoding(v: &Value) -> Vec<u8> {
    let mut e = Encoder::buffer(Vec::new());
    v.encode(&mut e);
    e.into_buffer()
}

/// The vectors, or `None` in a published crate, which does not include them.
fn vectors() -> Option<Json> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let path = root.join("../test-vectors/quiddity-v1.json");
    match std::fs::read_to_string(&path) {
        Ok(text) => Some(serde_json::from_str(&text).expect("the vectors file is valid JSON")),
        Err(_) if root.join(".cargo_vcs_info.json").exists() => {
            std::eprintln!("skipped: the test vectors are not part of the published crate");
            None
        }
        Err(err) => panic!("cannot read {}: {err}", path.display()),
    }
}

fn str_of<'a>(case: &'a Json, key: &str) -> &'a str {
    case[key]
        .as_str()
        .unwrap_or_else(|| panic!("{key} missing in {case}"))
}

#[test]
fn values() {
    let Some(vectors) = vectors() else { return };
    let cases = vectors["values"].as_array().unwrap();
    assert!(cases.len() > 50);
    for case in cases {
        let name = str_of(case, "name");
        let value = parse(&case["value"]);
        assert_eq!(
            to_hex(&encoding(&value)),
            str_of(case, "encoding"),
            "encoding of {name}"
        );
        let digest = value.quiddity();
        assert_eq!(
            digest.to_string(),
            str_of(case, "digest"),
            "digest of {name}"
        );
        assert_eq!(
            digest.mini().to_string(),
            str_of(case, "digest128"),
            "digest128 of {name}"
        );
        assert_eq!(
            digest.micro().to_string(),
            str_of(case, "digest64"),
            "digest64 of {name}"
        );
    }
}

#[test]
fn domains() {
    let Some(vectors) = vectors() else { return };
    for case in vectors["domains"].as_array().unwrap() {
        let name = str_of(case, "name");
        let structural = parse(&case["value"]).quiddity();
        assert_eq!(structural.to_string(), str_of(case, "structural"), "{name}");
        let digest = structural.in_domain(str_of(case, "label"));
        assert_eq!(digest.to_string(), str_of(case, "digest"), "{name}");
        assert_eq!(
            digest.mini().to_string(),
            str_of(case, "digest128"),
            "{name}"
        );
        assert_eq!(
            digest.micro().to_string(),
            str_of(case, "digest64"),
            "{name}"
        );
    }
}

/// JSON texts, mapped by spec 2.7.
#[cfg(feature = "serde_json")]
#[test]
fn json() {
    let Some(vectors) = vectors() else { return };
    for case in vectors["json"].as_array().unwrap() {
        let text = str_of(case, "text");
        let value: Json = serde_json::from_str(text).expect("valid JSON");
        assert_eq!(
            value.quiddity().to_string(),
            str_of(case, "digest"),
            "{text}"
        );
    }
}

#[test]
fn text_forms() {
    let Some(vectors) = vectors() else { return };
    for case in vectors["text_forms"].as_array().unwrap() {
        let text = str_of(case, "text");
        let valid = case["valid"].as_bool().unwrap();
        let parsed = [
            text.parse::<Digest>().map(|d| d.to_string()),
            text.parse::<Digest128>().map(|d| d.to_string()),
            text.parse::<Digest64>().map(|d| d.to_string()),
        ];
        let accepted: Vec<&String> = parsed.iter().filter_map(|r| r.as_ref().ok()).collect();
        if valid {
            assert_eq!(accepted.len(), 1, "{text} parses as exactly one size");
            assert_eq!(
                *accepted[0],
                text.to_ascii_lowercase(),
                "{text} round-trips"
            );
        } else {
            assert!(accepted.is_empty(), "{text} is rejected");
        }
    }
}

/// Identities (spec 5): each document's digest, which is computed with the
/// derive on `Identity` (spec 5.10), and each source value's encoding,
/// digest or error.
#[cfg(feature = "identity")]
#[test]
fn identities() {
    use crate::identity::Identity;
    let Some(vectors) = vectors() else { return };
    let cases = vectors["identities"].as_array().unwrap();
    assert!(cases.len() > 20);
    for case in cases {
        let name = str_of(case, "name");
        let identity =
            Identity::from_json(&case["document"]).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert_eq!(
            identity.quiddity().to_string(),
            str_of(case, "identity_digest"),
            "identity digest of {name}"
        );
        let reloaded = Identity::from_json(&identity.to_json()).unwrap();
        assert_eq!(reloaded, identity, "{name} round-trips");
        for c in case["cases"].as_array().unwrap() {
            let source = &c["source"];
            match identity.digest(source) {
                Ok(digest) => {
                    assert!(c.get("error").is_none(), "{name}: {source} must fail");
                    assert_eq!(
                        to_hex(&identity.encoding(source).unwrap()),
                        str_of(c, "encoding"),
                        "{name}: encoding of {source}"
                    );
                    assert_eq!(digest.to_string(), str_of(c, "digest"), "{name}: {source}");
                }
                Err(e) => assert!(c.get("error").is_some(), "{name}: {source} failed: {e}"),
            }
        }
    }
}

#[cfg(feature = "identity")]
#[test]
fn invalid_documents() {
    use crate::identity::Identity;
    let Some(vectors) = vectors() else { return };
    for case in vectors["invalid_documents"].as_array().unwrap() {
        assert!(
            Identity::from_json(&case["document"]).is_err(),
            "{} must be rejected",
            str_of(case, "name")
        );
    }
}

#[cfg(feature = "identity")]
#[test]
fn same_identity() {
    use crate::identity::Identity;
    let Some(vectors) = vectors() else { return };
    for case in vectors["same_identity"].as_array().unwrap() {
        for doc in case["documents"].as_array().unwrap() {
            let identity = Identity::from_json(doc).unwrap();
            assert_eq!(
                identity.quiddity().to_string(),
                str_of(case, "identity_digest"),
                "{}",
                str_of(case, "name")
            );
        }
    }
}

/// The identity of identity documents (spec Appendix C), under itself: its
/// digest computed with the derive equals its digest computed by applying
/// it, as an identity, to its own document.
#[cfg(feature = "identity")]
#[test]
fn identity_of_identities() {
    use crate::identity::Identity;
    let Some(vectors) = vectors() else { return };
    let case = &vectors["identity_of_identities"];
    let meta = Identity::from_json(&case["document"]).unwrap();
    let expected = str_of(case, "identity_digest");
    assert_eq!(meta.quiddity().to_string(), expected, "derived");
    assert_eq!(
        meta.digest(&meta.to_json()).unwrap().to_string(),
        expected,
        "applied to itself"
    );
    // The identity exported from the `Identity` type (spec 6.8) agrees.
    let exported = <Identity as crate::Quiddity>::construct_identity().unwrap();
    assert_eq!(
        exported.digest(&meta.to_json()).unwrap().to_string(),
        expected,
        "exported"
    );
    // And for every identity in the vectors.
    for c in vectors["identities"].as_array().unwrap() {
        let identity = Identity::from_json(&c["document"]).unwrap();
        for m in [&meta, &exported] {
            assert_eq!(
                m.digest(&identity.to_json()).unwrap(),
                identity.quiddity(),
                "{}",
                str_of(c, "name")
            );
        }
    }
}
