//! The optional features (spec 5.6 and 5.7).

#![cfg(feature = "std")]
// Shared items are unused when the features using them are off.
#![allow(unused_imports, dead_code)]

use std::collections::BTreeMap;

use quiddity::{Digest, Encoder, Quiddity, QuiddityExt};

/// `S({"start": 10, "end": 15})`, from the spec's Appendix A.
const START_END: &str = "sha256:ce5c58de07557be10cb78541577e3812bdb0b96b8af3aa17c4224753841bd989";

#[cfg(all(feature = "serde_json", feature = "derive"))]
mod serde_json_feature {
    use super::*;
    use serde_json::{Value, json};

    #[derive(Quiddity)]
    struct Interval {
        start: u64,
        end: u64,
    }

    fn parse(s: &str) -> Value {
        serde_json::from_str(s).unwrap()
    }

    #[test]
    fn objects_are_unordered() {
        assert_eq!(
            json!({"start": 10, "end": 15}).quiddity().to_string(),
            START_END
        );
        assert_eq!(
            parse(r#"{"end":15,"start":10}"#).quiddity().to_string(),
            START_END
        );
        assert_eq!(
            Interval { start: 10, end: 15 }.quiddity().to_string(),
            START_END
        );
    }

    #[test]
    fn numbers_by_value() {
        let one = 1u8.quiddity();
        assert_eq!(parse("1").quiddity(), one);
        assert_eq!(parse("1.0").quiddity(), one);
        assert_eq!(parse("1e0").quiddity(), one);
        assert_eq!(parse("-0.0").quiddity(), 0u8.quiddity());
        assert_eq!(parse("1.5").quiddity(), 1.5f64.quiddity());
        assert_eq!(
            parse("18446744073709551615").quiddity(),
            u64::MAX.quiddity()
        );
        assert_eq!(
            parse("-9223372036854775808").quiddity(),
            i64::MIN.quiddity()
        );
    }

    #[test]
    fn other_values() {
        assert_eq!(json!(null).quiddity(), None::<u8>.quiddity());
        assert_eq!(json!(true).quiddity(), true.quiddity());
        assert_eq!(json!("a").quiddity(), "a".quiddity());
        assert_eq!(json!([1, "a", null]).quiddity(), (1u8, "a", ()).quiddity());
        assert_ne!(json!([1, 2]).quiddity(), json!([2, 1]).quiddity());
        let map = json!({"a": [1]});
        assert_eq!(map.as_object().unwrap().quiddity(), map.quiddity());
    }

    /// The README's example, with a nested JSON value.
    #[derive(Quiddity)]
    struct Task {
        prompt: String,
        inputs: Vec<Value>,
        #[quiddity(skip)]
        submitted_at: std::time::SystemTime,
    }

    #[test]
    fn json_fields() {
        let task = Task {
            prompt: "p".into(),
            inputs: vec![json!({"k": 1.0})],
            submitted_at: std::time::SystemTime::now(),
        };
        assert_eq!(
            task.quiddity(),
            json!({"prompt": "p", "inputs": [{"k": 1}]}).quiddity()
        );
    }

    #[derive(Quiddity)]
    struct Annotated {
        id: u8,
        #[quiddity(omit_none)]
        extra: Value,
    }

    #[test]
    fn omit_none_on_json_null() {
        let a = Annotated {
            id: 1,
            extra: Value::Null,
        };
        assert_eq!(a.quiddity(), json!({"id": 1}).quiddity());
        let a = Annotated {
            id: 1,
            extra: json!([]),
        };
        assert_eq!(a.quiddity(), json!({"id": 1, "extra": []}).quiddity());
    }
}

#[cfg(all(feature = "serde", feature = "derive"))]
mod serde_feature {
    use super::*;
    use serde::{Serialize, Serializer};

    #[derive(Serialize, Quiddity)]
    struct Interval {
        start: u64,
        end: u64,
    }

    #[derive(Serialize, Quiddity)]
    enum Shape {
        Empty,
        Circle(u32),
        Rect(u32, u32),
        Poly { sides: u8, label: Option<String> },
    }

    #[derive(Serialize, Quiddity)]
    struct Everything {
        unit: (),
        tuple: (u8, String),
        newtype: Newtype,
        unit_struct: UnitStruct,
        shapes: Vec<Shape>,
        keys: BTreeMap<u8, bool>,
        big: i128,
        ch: char,
        float: f32,
        #[serde(skip_serializing_if = "Option::is_none")]
        #[quiddity(omit_none)]
        gone: Option<u8>,
    }

    #[derive(Serialize, Quiddity)]
    struct Newtype(u16);

    #[derive(Serialize, Quiddity)]
    struct UnitStruct;

    /// Every `rename_all` convention, on fields and on variants, gives the
    /// names serde gives.
    macro_rules! same_renames {
        ($($rule:literal $s:ident $e:ident;)*) => {$(
            #[derive(Serialize, Quiddity)]
            #[serde(rename_all = $rule)]
            #[quiddity(rename_all = $rule)]
            struct $s {
                user_id: u8,
                x: u8,
                http_status_code: u8,
                a_b_c: u8,
            }

            #[derive(Serialize, Quiddity)]
            #[serde(rename_all = $rule)]
            #[quiddity(rename_all = $rule)]
            enum $e {
                PageView,
                X,
                HTTPStatus,
                FormSubmit { form_id: u8 },
            }
        )*

            #[test]
            fn rename_all_matches_serde() {$(
                let s = $s { user_id: 1, x: 2, http_status_code: 3, a_b_c: 4 };
                assert_eq!(quiddity::serde::digest(&s).unwrap(), s.quiddity(), "{}", $rule);
                for e in [$e::PageView, $e::X, $e::HTTPStatus, $e::FormSubmit { form_id: 5 }] {
                    assert_eq!(quiddity::serde::digest(&e).unwrap(), e.quiddity(), "{}", $rule);
                }
            )*}
        };
    }

    same_renames! {
        "lowercase" Lower LowerE;
        "UPPERCASE" Upper UpperE;
        "PascalCase" Pascal PascalE;
        "camelCase" Camel CamelE;
        "snake_case" Snake SnakeE;
        "SCREAMING_SNAKE_CASE" ScreamingSnake ScreamingSnakeE;
        "kebab-case" Kebab KebabE;
        "SCREAMING-KEBAB-CASE" ScreamingKebab ScreamingKebabE;
    }

    #[test]
    fn same_as_native() {
        let i = Interval { start: 10, end: 15 };
        assert_eq!(quiddity::serde::digest(&i).unwrap().to_string(), START_END);
        let e = Everything {
            unit: (),
            tuple: (1, "a".into()),
            newtype: Newtype(7),
            unit_struct: UnitStruct,
            shapes: vec![
                Shape::Empty,
                Shape::Circle(1),
                Shape::Rect(1, 2),
                Shape::Poly {
                    sides: 3,
                    label: None,
                },
            ],
            keys: BTreeMap::from([(2, true), (1, false)]),
            big: i128::MIN,
            ch: 'x',
            float: 0.5,
            gone: None,
        };
        assert_eq!(quiddity::serde::digest(&e).unwrap(), e.quiddity());
    }

    struct Raw(&'static [u8]);

    impl Serialize for Raw {
        fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
            s.serialize_bytes(self.0)
        }
    }

    #[test]
    fn bytes_and_strings() {
        assert_eq!(
            quiddity::serde::digest(&Raw(&[1, 2])).unwrap(),
            BytesValue(&[1, 2]).quiddity()
        );
        // Text and bytes are one kind.
        assert_eq!(
            quiddity::serde::digest(&Raw(b"ab")).unwrap(),
            "ab".quiddity()
        );
        let addr = std::net::Ipv4Addr::LOCALHOST;
        assert_eq!(
            quiddity::serde::digest(&addr).unwrap(),
            "127.0.0.1".quiddity()
        );
        assert_eq!(quiddity::serde::digest(&addr).unwrap(), addr.quiddity());
    }

    struct BytesValue(&'static [u8]);

    impl Quiddity for BytesValue {
        fn encode(&self, e: &mut Encoder) {
            e.bytes(self.0);
        }
    }

    struct Failing;

    impl Serialize for Failing {
        fn serialize<S: Serializer>(&self, _: S) -> Result<S::Ok, S::Error> {
            Err(serde::ser::Error::custom("no"))
        }
    }

    #[derive(Quiddity)]
    struct HasFailing {
        #[quiddity(encode_as = serde)]
        f: Failing,
    }

    #[derive(Quiddity)]
    struct ViaSerde {
        #[quiddity(encode_as = serde)]
        at: Interval,
    }

    #[test]
    fn encode_as_serde() {
        let v = ViaSerde {
            at: Interval { start: 10, end: 15 },
        };
        let mut m = BTreeMap::new();
        m.insert("at", BTreeMap::from([("start", 10u8), ("end", 15)]));
        assert_eq!(v.quiddity(), m.quiddity());
    }

    #[test]
    fn errors() {
        assert_eq!(
            quiddity::serde::digest(&Failing).unwrap_err().to_string(),
            "no"
        );
    }

    #[test]
    #[should_panic(expected = "a `Serialize` impl failed")]
    fn errors_panic_inside_quiddity() {
        let _ = HasFailing { f: Failing }.quiddity();
    }

    #[test]
    fn digests_serialize_as_text() {
        let d = 5u8.quiddity();
        let json = serde_json::to_string(&d).unwrap();
        assert_eq!(json, format!("\"{d}\""));
        assert_eq!(serde_json::from_str::<Digest>(&json).unwrap(), d);
        let short = serde_json::to_string(&d.micro()).unwrap();
        assert_eq!(
            serde_json::from_str::<quiddity::Digest64>(&short).unwrap(),
            d.micro()
        );
        assert!(serde_json::from_str::<Digest>(&short).is_err());
    }
}

#[cfg(feature = "indexmap")]
#[test]
fn indexmap() {
    let m = indexmap::IndexMap::from([("start", 10u8), ("end", 15)]);
    assert_eq!(m.quiddity().to_string(), START_END);
    let s = indexmap::IndexSet::from([2u8, 1]);
    assert_eq!(
        s.quiddity(),
        std::collections::BTreeSet::from([1u8, 2]).quiddity()
    );
}

#[cfg(feature = "uuid")]
#[test]
fn uuid() {
    let text = "67e55044-10b1-426f-9247-bb680e5fe0c8";
    let id: uuid::Uuid = text.parse().unwrap();
    assert_eq!(id.quiddity(), text.quiddity());
}

#[cfg(feature = "bytes")]
#[test]
fn bytes() {
    let b = bytes::Bytes::from_static(&[1, 2]);
    let m = bytes::BytesMut::from(&[1u8, 2][..]);
    assert_eq!(b.quiddity(), m.quiddity());
    assert_ne!(b.quiddity(), vec![1u8, 2].quiddity());
    assert_eq!(bytes::Bytes::from_static(b"ab").quiddity(), "ab".quiddity());
}

#[cfg(feature = "url")]
#[test]
fn url() {
    let u: url::Url = "https://example.com/a?b=1".parse().unwrap();
    assert_eq!(u.quiddity(), "https://example.com/a?b=1".quiddity());
}
