//! The standard library mappings of spec 5.2, and the equivalences the README
//! states for them.

#![cfg(feature = "std")]

use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet, VecDeque};
use std::marker::PhantomData;
use std::num::NonZero;
use std::rc::Rc;
use std::sync::Arc;

use quiddity::{Digest, Encoder, IdentityMarker, Primary, Quiddity, QuiddityExt};

/// `S({"start": 10, "end": 15})`, from the spec's Appendix A.
const START_END: &str = "sha256:ce5c58de07557be10cb78541577e3812bdb0b96b8af3aa17c4224753841bd989";

fn same<A: Quiddity + ?Sized, B: Quiddity + ?Sized>(a: &A, b: &B) -> bool {
    a.quiddity() == b.quiddity()
}

#[test]
fn numbers_by_value() {
    assert!(same(&5u8, &5i64));
    assert!(same(&5u8, &5u128));
    assert!(same(&5u8, &5.0f64));
    assert!(same(&5u8, &5.0f32));
    assert!(same(&5u8, &NonZero::new(5u8).unwrap()));
    assert!(same(&-5i8, &NonZero::new(-5i128).unwrap()));
    assert!(same(&-0.0f64, &0u8));
    assert!(same(&f64::NAN, &f32::NAN));
    assert!(same(&0.5f32, &0.5f64));
    assert!(!same(&0.1f32, &0.1f64));
    assert!(!same(&true, &1u8));
    assert!(!same(&false, &0u8));
}

#[test]
fn strings() {
    let s = "a";
    assert!(same(&'a', s));
    assert!(same(s, &String::from(s)));
    assert!(same(s, &Box::<str>::from(s)));
    assert!(same(s, &Rc::<str>::from(s)));
    assert!(same(s, &Arc::<str>::from(s)));
    assert!(same(s, &Cow::<str>::Borrowed(s)));
    assert!(same(s, &Cow::<str>::Owned(s.to_owned())));
    assert!(same(&'é', "\u{e9}"));
    assert!(!same("\u{e9}", "e\u{301}"));
}

#[test]
fn nulls() {
    assert!(same(&(), &None::<u8>));
    assert!(same(&(), &PhantomData::<String>));
    assert!(same(&Some(5u8), &5u8));
    assert!(same(&Some(Some(5u8)), &5u8));
    // The documented collapses.
    assert!(same(&Some(None::<u8>), &None::<Option<u8>>));
    assert!(same(&Some(()), &None::<()>));
}

#[test]
fn lists() {
    let v = vec![1u8, 2];
    assert!(same(&v, &[1u8, 2]));
    assert!(same(&v, &(1u32, 2i64)));
    assert!(same(&v, &VecDeque::from([1u16, 2])));
    assert!(same(&v, &v[..]));
    assert!(same(&v, &v.clone().into_boxed_slice()));
    assert!(same(&v, &[1.0f64, 2.0]));
    assert!(!same(&v, &vec![2u8, 1]));
    assert!(same(&(1u8,), &[1u8]));
    assert!(same(&Vec::<u8>::new(), &[0u8; 0]));
    // Lists of different kinds of value.
    assert!(same(
        &(1u8, "a", ()),
        &(1i32, String::from("a"), None::<bool>)
    ));
}

#[test]
fn sets() {
    let b = BTreeSet::from([1u8, 2]);
    assert!(same(&b, &HashSet::from([2u64, 1])));
    assert!(!same(&b, &vec![1u8, 2]));
    // Members are encoded by value, whatever their type.
    assert!(same(&BTreeSet::from([1u8]), &HashSet::from([1u64])));
}

#[test]
fn maps() {
    let b = BTreeMap::from([("start", 10u64), ("end", 15)]);
    assert_eq!(b.quiddity().to_string(), START_END);
    let h = HashMap::from([(String::from("end"), 15u8), (String::from("start"), 10)]);
    assert_eq!(h.quiddity().to_string(), START_END);
    // Non-string keys.
    assert!(same(
        &BTreeMap::from([(1u8, true)]),
        &HashMap::from([(1i64, true)])
    ));
}

#[test]
fn digests_as_strings() {
    let d = 5u8.quiddity();
    assert!(same(&d, &d.to_string()));
    assert!(same(&d.mini(), &d.mini().to_string()));
    assert!(same(&d.micro(), &d.micro().to_string()));
}

/// The README's manual implementation example.
pub struct Interval {
    start: u64,
    len: u64,
}

impl Quiddity for Interval {
    const DOMAIN: Option<&'static str> = None; // the default; may be omitted

    fn encode(&self, e: &mut Encoder) {
        let mut f = e.fields();
        // Fields must be written in sorted order (checked in debug builds).
        f.field("end", &(self.start + self.len));
        f.field("start", &self.start);
        f.finish();
    }
}

#[test]
fn manual_impl() {
    let i = Interval { start: 10, len: 5 };
    assert_eq!(i.quiddity().to_string(), START_END);
}

/// A type with a domain, implemented by hand.
struct Payment {
    to: &'static str,
    amount: u64,
}

impl Quiddity for Payment {
    const DOMAIN: Option<&'static str> = Some("example.payment");

    fn encode(&self, e: &mut Encoder) {
        let mut f = e.fields();
        f.field("amount", &self.amount);
        f.field("to", self.to);
    }
}

/// `Payment`'s fields, without a domain.
struct Plain {
    to: &'static str,
    amount: u64,
}

impl Quiddity for Plain {
    fn encode(&self, e: &mut Encoder) {
        let mut f = e.fields();
        f.field("amount", &self.amount);
        f.field("to", self.to);
    }
}

#[test]
fn domains() {
    let p = Payment {
        to: "bob",
        amount: 100,
    };
    // The spec's Appendix A domain example.
    assert_eq!(
        p.quiddity().to_string(),
        "sha256:f9104f8b85740d8c6c47c0f5a04bb05d264ee25f9be3d310be66385b298da830"
    );
    assert_eq!(
        p.quiddity().micro().to_string(),
        "sha256-64:f9104f8b85740d8c"
    );
    // Raising the structural digest of the same fields into the domain.
    let plain = Plain {
        to: p.to,
        amount: p.amount,
    };
    assert_eq!(plain.quiddity().in_domain("example.payment"), p.quiddity());
    // Pointers carry the domain of what they point to.
    assert_eq!(Box::new(&p).quiddity(), p.quiddity());
    let expected = p.quiddity();
    assert_eq!(Rc::new(p).quiddity(), expected);
    // Nested values contribute their encoding, never their domain.
    let nested = vec![Payment {
        to: "bob",
        amount: 100,
    }];
    let plain = vec![BTreeMap::from([("to", "bob")])];
    assert_ne!(nested.quiddity(), plain.quiddity());
}

#[derive(Debug)]
struct Meaning;
impl IdentityMarker for Meaning {}

/// A container implementing every identity, as in the README.
struct Ring<T>(Vec<T>);

impl<I: IdentityMarker, T: Quiddity<I>> Quiddity<I> for Ring<T> {
    fn encode(&self, e: &mut Encoder) {
        let mut l = e.list();
        for item in &self.0 {
            l.item_as::<I, _>(item);
        }
        l.finish();
    }
}

#[test]
fn identities_reach_through_containers() {
    assert_eq!(vec![1u8].quiddity_as::<Meaning>(), vec![1u8].quiddity());
    assert_eq!(
        Ring(vec![1u8, 2]).quiddity_as::<Meaning>(),
        vec![1, 2].quiddity()
    );
    assert_eq!(
        Ring(vec![1u8]).quiddity_as::<Primary>(),
        Ring(vec![1u8]).quiddity()
    );
}

#[test]
fn text_forms_round_trip() {
    let d = "x".quiddity();
    assert_eq!(d.to_string().parse::<Digest>().unwrap(), d);
    assert_eq!(
        d.to_string()
            .to_uppercase()
            .replace("SHA256", "sha256")
            .parse::<Digest>()
            .unwrap(),
        d
    );
    assert!(d.mini().to_string().parse::<Digest>().is_err());
    assert_eq!(format!("{d:?}"), format!("Digest(\"{d}\")"));
}
