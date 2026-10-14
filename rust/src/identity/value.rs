//! Data-model values (spec 2.1) produced by applying an identity, and the
//! exact arithmetic that conversions need.

use alloc::vec;
use alloc::vec::Vec;

use crate::{Encoder, Quiddity};

/// A data-model value.
#[derive(Debug)]
pub(crate) enum Val {
    Null,
    Bool(bool),
    /// An integer, as big-endian two's-complement bytes of any length.
    Int(Vec<u8>),
    /// A number from a binary64 value, normalized when encoded (spec 2.2).
    Float(f64),
    /// A string's bytes.
    Str(Vec<u8>),
    List(Vec<Val>),
    Set(Vec<Val>),
    Map(Vec<(Val, Val)>),
}

impl Val {
    pub(crate) fn int(n: i128) -> Val {
        Val::Int(n.to_be_bytes().to_vec())
    }

    pub(crate) fn str(s: &str) -> Val {
        Val::Str(s.as_bytes().to_vec())
    }

    pub(crate) fn is_null(&self) -> bool {
        matches!(self, Val::Null)
    }

    /// An empty string, list, set or map.
    pub(crate) fn is_empty(&self) -> bool {
        match self {
            Val::Str(s) => s.is_empty(),
            Val::List(v) | Val::Set(v) => v.is_empty(),
            Val::Map(m) => m.is_empty(),
            Val::Null | Val::Bool(_) | Val::Int(_) | Val::Float(_) => false,
        }
    }
}

impl Quiddity for Val {
    fn encode(&self, e: &mut Encoder) {
        match self {
            Val::Null => e.null(),
            Val::Bool(b) => e.bool(*b),
            Val::Int(bytes) => e.int_twos_complement(bytes),
            Val::Float(x) => e.float(*x),
            Val::Str(s) => e.bytes(s),
            Val::List(items) => {
                let mut l = e.list();
                for item in items {
                    l.item(item);
                }
            }
            Val::Set(members) => {
                let mut s = e.set();
                for m in members {
                    s.member(m);
                }
            }
            Val::Map(entries) => {
                let mut m = e.map();
                for (k, v) in entries {
                    m.entry(k, v);
                }
            }
        }
    }
}

/// The most decimal digits an integer from text may have (spec 5.5 allows a
/// limit), so that `1e999999999` cannot exhaust memory.
const MAX_DIGITS: usize = 4096;

/// Decimal text (spec 5.5): the syntax of a JSON number, optionally with a
/// leading `+`. Its value is `digits × 10^exponent`, negated if `negative`.
pub(crate) struct Decimal<'a> {
    pub(crate) negative: bool,
    /// The integer and fraction digits, without the point.
    digits: Vec<u8>,
    exponent: i64,
    /// The text without a leading `+`, for parsing as a float.
    pub(crate) body: &'a str,
    /// Whether the text has neither a fraction nor an exponent.
    pub(crate) plain_integer: bool,
}

impl<'a> Decimal<'a> {
    pub(crate) fn parse(text: &'a str) -> Option<Decimal<'a>> {
        let body = text.strip_prefix('+').unwrap_or(text);
        let b = body.as_bytes();
        let mut i = 0;
        let negative = b.first() == Some(&b'-');
        if negative {
            i += 1;
        }
        let int_start = i;
        match b.get(i) {
            Some(b'0') => i += 1,
            Some(c) if c.is_ascii_digit() => {
                while b.get(i).is_some_and(u8::is_ascii_digit) {
                    i += 1;
                }
            }
            _ => return None,
        }
        let mut digits = b[int_start..i].to_vec();
        let mut exponent: i64 = 0;
        let mut plain_integer = true;
        if b.get(i) == Some(&b'.') {
            i += 1;
            let start = i;
            while b.get(i).is_some_and(u8::is_ascii_digit) {
                i += 1;
            }
            if i == start {
                return None;
            }
            digits.extend_from_slice(&b[start..i]);
            exponent -= (i - start) as i64;
            plain_integer = false;
        }
        if matches!(b.get(i), Some(b'e' | b'E')) {
            i += 1;
            let sign = match b.get(i) {
                Some(b'-') => {
                    i += 1;
                    -1
                }
                Some(b'+') => {
                    i += 1;
                    1
                }
                _ => 1,
            };
            let start = i;
            while b.get(i).is_some_and(u8::is_ascii_digit) {
                i += 1;
            }
            if i == start {
                return None;
            }
            let e: i64 = body[start..i].parse().ok()?;
            exponent = exponent.checked_add(sign * e)?;
            plain_integer = false;
        }
        (i == b.len()).then_some(Decimal {
            negative,
            digits,
            exponent,
            body,
            plain_integer,
        })
    }

    /// The value times `10^scale`, as two's-complement bytes, if it is an
    /// integer. `Err` if it is an integer too large to convert.
    pub(crate) fn integer(&self, scale: u32) -> Result<Option<Vec<u8>>, ()> {
        let exponent = self.exponent + i64::from(scale);
        let mut digits = self.digits.clone();
        if exponent >= 0 {
            let zeros = usize::try_from(exponent).map_err(|_| ())?;
            if digits.iter().all(|&d| d == b'0') {
                digits = vec![b'0'];
            } else if digits.len() + zeros > MAX_DIGITS {
                return Err(());
            } else {
                digits.resize(digits.len() + zeros, b'0');
            }
        } else {
            let drop = usize::try_from(-exponent).unwrap_or(usize::MAX);
            let keep = digits.len().saturating_sub(drop);
            if digits[keep..].iter().any(|&d| d != b'0') {
                return Ok(None);
            }
            digits.truncate(keep);
            if digits.is_empty() {
                digits.push(b'0');
            }
        }
        Ok(Some(decimal_to_twos_complement(&digits, self.negative)))
    }
}

/// Decimal digits as big-endian two's-complement bytes.
pub(crate) fn decimal_to_twos_complement(digits: &[u8], negative: bool) -> Vec<u8> {
    // The magnitude, big-endian, with a leading zero byte for the sign.
    let mut bytes = vec![0u8];
    for d in digits {
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

/// `x × 10^scale` as an integer, if it is one and fits in an `i128`.
/// `Err(())` if `x` is not finite or the result does not fit.
pub(crate) fn scaled_float(x: f64, scale: u32) -> Result<Option<i128>, ()> {
    if !x.is_finite() {
        return Err(());
    }
    if x == 0.0 {
        return Ok(Some(0));
    }
    let bits = x.to_bits();
    let biased = ((bits >> 52) & 0x7FF) as i32;
    let fraction = bits & ((1 << 52) - 1);
    // x = mantissa × 2^exp2 exactly.
    let (mantissa, exp2) = if biased == 0 {
        (fraction, -1074)
    } else {
        (fraction | (1 << 52), biased - 1075)
    };
    // x × 10^scale = mantissa × 5^scale × 2^(exp2 + scale).
    let m = i128::from(mantissa) * 5i128.pow(scale);
    let shift = exp2 + scale as i32;
    let value = if shift >= 0 {
        if shift >= 127 || m.leading_zeros() <= shift as u32 {
            return Err(());
        }
        m << shift
    } else {
        let down = (-shift) as u32;
        if down >= 128 || m.trailing_zeros() < down {
            return Ok(None);
        }
        m >> down
    };
    Ok(Some(if x < 0.0 { -value } else { value }))
}
