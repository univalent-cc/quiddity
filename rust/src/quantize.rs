//! Quantization (spec 5.6): a value rounded down to a multiple of a step,
//! computed exactly, without allocating.

use core::fmt::Write as _;

use crate::Encoder;
use crate::number::negate;

/// The bytes needed for any integer this module produces: an `i128` or
/// `u128` quantized, or a float's quantized value (at most about 10^317).
pub(crate) const INT_BUF: usize = 160;

/// `v`, an integer as big-endian two's-complement bytes of any length,
/// rounded down to a multiple of `q`, written into `out` as two's-complement
/// bytes. `out` must be at least `v.len() + 9` bytes long.
pub(crate) fn int<'o>(v: &[u8], q: u64, out: &'o mut [u8]) -> &'o [u8] {
    let len = out.len();
    assert!(len >= v.len() + 9, "quantize buffer too small");
    let negative = v.first().is_some_and(|b| b & 0x80 != 0);
    let fill = if negative { 0xFF } else { 0x00 };
    out[..len - v.len()].fill(fill);
    out[len - v.len()..].copy_from_slice(v);
    // The magnitude, unsigned.
    if negative {
        negate(out);
    }
    let r = out
        .iter()
        .fold(0u128, |r, &b| (r * 256 + u128::from(b)) % u128::from(q)) as u64;
    if negative {
        // Toward negative infinity: a larger magnitude.
        if r != 0 {
            add(out, q - r);
        }
        negate(out);
    } else {
        sub(out, r);
    }
    out
}

fn add(bytes: &mut [u8], mut n: u64) {
    for b in bytes.iter_mut().rev() {
        if n == 0 {
            break;
        }
        let sum = u64::from(*b) + (n & 0xFF);
        *b = sum as u8;
        n = (n >> 8) + (sum >> 8);
    }
}

fn sub(bytes: &mut [u8], mut n: u64) {
    let mut borrow = 0u64;
    for b in bytes.iter_mut().rev() {
        if n == 0 && borrow == 0 {
            break;
        }
        let take = (n & 0xFF) + borrow;
        let have = u64::from(*b);
        if have >= take {
            *b = (have - take) as u8;
            borrow = 0;
        } else {
            *b = (have + 256 - take) as u8;
            borrow = 1;
        }
        n >>= 8;
    }
}

/// A float's quantized value (spec 5.6).
#[derive(Debug, PartialEq)]
pub(crate) enum Decimal {
    /// A number, to be normalized when encoded (spec 2.2).
    Float(f64),
    /// The integer `n × 10^zeros`.
    Int { n: i128, zeros: u32 },
}

/// A writer into a fixed buffer.
struct Buf<const N: usize> {
    bytes: [u8; N],
    len: usize,
}

impl<const N: usize> core::fmt::Write for Buf<N> {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        let end = self.len + s.len();
        if end > N {
            return Err(core::fmt::Error);
        }
        self.bytes[self.len..end].copy_from_slice(s.as_bytes());
        self.len = end;
        Ok(())
    }
}

impl<const N: usize> Buf<N> {
    fn new() -> Self {
        Buf {
            bytes: [0; N],
            len: 0,
        }
    }

    fn as_str(&self) -> &str {
        core::str::from_utf8(&self.bytes[..self.len]).unwrap()
    }
}

/// `x` rounded down to a multiple of `10^−d`, on the shortest decimal that
/// rounds to `x` (Rust's `{:e}` formatting); `x` is finite, and is an `f32`
/// or `f64` formatted as such.
fn decimal(shortest: &str, x_is_on_grid: impl FnOnce() -> Decimal, d: i32) -> Decimal {
    // `shortest` is `[-]D[.DDD]e[-]E`.
    let (negative, s) = match shortest.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, shortest),
    };
    let (mantissa, exp) = s.split_once('e').unwrap();
    let exp: i32 = exp.parse().unwrap();
    let digits = mantissa_digits(mantissa);
    let fraction_digits = mantissa.split_once('.').map_or(0, |(_, f)| f.len() as i32);
    // x = digits × 10^e.
    let e = exp - fraction_digits;
    if e + d >= 0 {
        return x_is_on_grid();
    }
    let k = -(e + d);
    let (whole, exact) = if k > 19 {
        (0u64, digits == 0)
    } else {
        let p = 10u64.pow(k as u32);
        (digits / p, digits % p == 0)
    };
    let n: i128 = if negative {
        -(i128::from(whole) + i128::from(!exact))
    } else {
        i128::from(whole)
    };
    if d >= 0 {
        // n × 10^−d: below 2^53 in magnitude, since x has a fraction.
        let mut b = Buf::<64>::new();
        write!(b, "{n}e-{d}").unwrap();
        Decimal::Float(b.as_str().parse().unwrap())
    } else {
        Decimal::Int {
            n,
            zeros: d.unsigned_abs(),
        }
    }
}

/// The digits of a decimal mantissa (`1.2345`), without the point.
fn mantissa_digits(m: &str) -> u64 {
    m.bytes()
        .filter(u8::is_ascii_digit)
        .fold(0u64, |acc, c| acc * 10 + u64::from(c - b'0'))
}

/// An `f64` quantized to `d` decimals.
pub(crate) fn f64(x: f64, d: i32) -> Decimal {
    if !x.is_finite() {
        return Decimal::Float(x);
    }
    let mut b = Buf::<40>::new();
    write!(b, "{x:e}").unwrap();
    decimal(b.as_str(), || Decimal::Float(x), d)
}

/// An `f32` quantized to `d` decimals, on its shortest decimal as a binary32.
pub(crate) fn f32(x: f32, d: i32) -> Decimal {
    if !x.is_finite() {
        return Decimal::Float(f64::from(x));
    }
    let mut b = Buf::<40>::new();
    write!(b, "{x:e}").unwrap();
    let s = b.as_str();
    // On the grid: the binary32 value's shortest decimal, as binary64.
    let on_grid = || {
        let mut c = Buf::<40>::new();
        c.write_str(s).unwrap();
        Decimal::Float(c.as_str().parse().unwrap())
    };
    decimal(s, on_grid, d)
}

/// Writes a quantized float's value.
pub(crate) fn encode_decimal(v: Decimal, e: &mut Encoder) {
    match v {
        Decimal::Float(x) => e.float(x),
        Decimal::Int { n, zeros } => {
            let mut bytes = [0u8; INT_BUF];
            e.int_twos_complement(scaled(n, zeros, &mut bytes));
        }
    }
}

/// `n × 10^zeros` as two's-complement bytes.
pub(crate) fn scaled(n: i128, zeros: u32, buf: &mut [u8; INT_BUF]) -> &[u8] {
    let negative = n < 0;
    let magnitude = n.unsigned_abs();
    buf.fill(0);
    // The magnitude, big-endian, multiplied in place.
    let m = magnitude.to_be_bytes();
    buf[INT_BUF - 16..].copy_from_slice(&m);
    for _ in 0..zeros {
        let mut carry = 0u32;
        for b in buf.iter_mut().rev() {
            let v = u32::from(*b) * 10 + carry;
            *b = v as u8;
            carry = v >> 8;
        }
    }
    if negative {
        negate(buf);
    }
    buf
}

/// Quantizes a field's value for the derive's `quantize` (spec 6.5).
/// Implemented for the types it applies to, so that it fails to compile on
/// any other.
#[diagnostic::on_unimplemented(
    message = "`quantize` applies to integers, `Duration` and `SystemTime` \
               (or an `Option` of one), not `{Self}`",
    label = "cannot be quantized"
)]
pub trait Quantize {
    /// Writes the value rounded down to a multiple of `step`.
    fn encode_quantized(&self, step: u64, e: &mut Encoder);
}

/// Quantizes a field's value for the derive's `decimals` (spec 6.5).
#[diagnostic::on_unimplemented(
    message = "`decimals` applies to `f32` and `f64` (or an `Option` of one), not `{Self}`",
    label = "cannot be quantized to decimals"
)]
pub trait Decimals {
    /// Writes the value rounded down to a multiple of 10^−`decimals`.
    fn encode_decimals(&self, decimals: i32, e: &mut Encoder);
}

fn encode_int(twos: &[u8], step: u64, e: &mut Encoder) {
    let mut out = [0u8; 32];
    let n = twos.len() + 9;
    e.int_twos_complement(int(twos, step, &mut out[..n]));
}

macro_rules! quantize_integers {
    ($($t:ty)*) => {$(
        impl Quantize for $t {
            fn encode_quantized(&self, step: u64, e: &mut Encoder) {
                let mut buf = [0u8; crate::number::INT_BUF];
                encode_int(crate::number::Integer::twos_complement(*self, &mut buf), step, e);
            }
        }

        impl Quantize for core::num::NonZero<$t> {
            fn encode_quantized(&self, step: u64, e: &mut Encoder) {
                self.get().encode_quantized(step, e);
            }
        }
    )*};
}

quantize_integers!(i8 i16 i32 i64 i128 isize u8 u16 u32 u64 u128 usize);

/// Integer nanoseconds (spec 2.6).
impl Quantize for core::time::Duration {
    fn encode_quantized(&self, step: u64, e: &mut Encoder) {
        self.as_nanos().encode_quantized(step, e);
    }
}

/// Integer nanoseconds since the Unix epoch (spec 2.6).
#[cfg(feature = "std")]
impl Quantize for std::time::SystemTime {
    fn encode_quantized(&self, step: u64, e: &mut Encoder) {
        let ns = match self.duration_since(std::time::UNIX_EPOCH) {
            Ok(after) => after.as_nanos() as i128,
            Err(before) => -(before.duration().as_nanos() as i128),
        };
        ns.encode_quantized(step, e);
    }
}

impl<T: Quantize> Quantize for Option<T> {
    fn encode_quantized(&self, step: u64, e: &mut Encoder) {
        match self {
            Some(v) => v.encode_quantized(step, e),
            None => e.null(),
        }
    }
}

impl<T: Quantize + ?Sized> Quantize for &T {
    fn encode_quantized(&self, step: u64, e: &mut Encoder) {
        T::encode_quantized(self, step, e);
    }
}

impl Decimals for f64 {
    fn encode_decimals(&self, decimals: i32, e: &mut Encoder) {
        encode_decimal(f64(*self, decimals), e);
    }
}

impl Decimals for f32 {
    fn encode_decimals(&self, decimals: i32, e: &mut Encoder) {
        encode_decimal(f32(*self, decimals), e);
    }
}

impl<T: Decimals> Decimals for Option<T> {
    fn encode_decimals(&self, decimals: i32, e: &mut Encoder) {
        match self {
            Some(v) => v.encode_decimals(decimals, e),
            None => e.null(),
        }
    }
}

impl<T: Decimals + ?Sized> Decimals for &T {
    fn encode_decimals(&self, decimals: i32, e: &mut Encoder) {
        T::encode_decimals(self, decimals, e);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn int_of(v: i128, q: u64) -> i128 {
        let mut out = [0u8; 25];
        let r = int(&v.to_be_bytes(), q, &mut out);
        let r = crate::number::minimal(r);
        let mut buf = [if r[0] & 0x80 != 0 { 0xFF } else { 0 }; 16];
        buf[16 - r.len()..].copy_from_slice(r);
        i128::from_be_bytes(buf)
    }

    #[test]
    fn integers() {
        assert_eq!(int_of(149, 100), 100);
        assert_eq!(int_of(100, 100), 100);
        assert_eq!(int_of(99, 100), 0);
        assert_eq!(int_of(-1, 100), -100);
        assert_eq!(int_of(-100, 100), -100);
        assert_eq!(int_of(-101, 100), -200);
        assert_eq!(int_of(i128::MIN, 2), i128::MIN);
        assert_eq!(int_of(i128::MIN + 1, 2), i128::MIN);
        assert_eq!(int_of(i128::MAX, 2), i128::MAX - 1);
        // Below `i128::MIN`: one more byte.
        let mut out = [0u8; 25];
        let r = crate::number::minimal(int(&i128::MIN.to_be_bytes(), 3, &mut out)).len();
        assert_eq!(r, 17);
    }

    #[test]
    fn floats() {
        assert_eq!(f64(12.989, 2), Decimal::Float(12.98));
        assert_eq!(f64(2.675, 2), Decimal::Float(2.67));
        assert_eq!(f64(2.67, 2), Decimal::Float(2.67));
        assert_eq!(f64(-0.001, 2), Decimal::Float(-0.01));
        assert_eq!(f64(0.1 + 0.2, 2), Decimal::Float(0.3));
        assert_eq!(f64(1234.5, -2), Decimal::Int { n: 12, zeros: 2 });
        assert_eq!(f64(-0.5, -2), Decimal::Int { n: -1, zeros: 2 });
        assert_eq!(f64(1e300, -2), Decimal::Float(1e300));
        assert_eq!(f32(0.7, 1), Decimal::Float(0.7));
    }
}
