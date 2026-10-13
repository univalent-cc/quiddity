//! Integer and float encoding (spec 2.2 and 3.4).

/// Every NaN is encoded as this binary64 value.
pub(crate) const CANONICAL_NAN: u64 = 0x7FF8_0000_0000_0000;

/// Enough for any primitive integer: a sign byte and 16 bytes.
pub(crate) const INT_BUF: usize = 17;

/// Enough for any integral `f64`: a sign byte, 8 bytes of mantissa, and
/// 121 bytes of shift (the largest exponent is 2^971 times the mantissa).
pub(crate) const FLOAT_INT_BUF: usize = 130;

mod sealed {
    pub trait Sealed {}
}

/// The primitive integer types, which [`Encoder::int`](crate::Encoder::int)
/// accepts. Sealed: it cannot be implemented outside this crate.
pub trait Integer: sealed::Sealed + Copy {
    #[doc(hidden)]
    fn twos_complement(self, buf: &mut [u8; INT_BUF]) -> &[u8];
}

macro_rules! signed {
    ($($t:ty)*) => {$(
        impl sealed::Sealed for $t {}
        impl Integer for $t {
            fn twos_complement(self, buf: &mut [u8; INT_BUF]) -> &[u8] {
                buf[1..].copy_from_slice(&(self as i128).to_be_bytes());
                minimal(&buf[1..])
            }
        }
    )*};
}

macro_rules! unsigned {
    ($($t:ty)*) => {$(
        impl sealed::Sealed for $t {}
        impl Integer for $t {
            fn twos_complement(self, buf: &mut [u8; INT_BUF]) -> &[u8] {
                buf[0] = 0;
                buf[1..].copy_from_slice(&(self as u128).to_be_bytes());
                minimal(buf)
            }
        }
    )*};
}

signed!(i8 i16 i32 i64 i128 isize);
unsigned!(u8 u16 u32 u64 u128 usize);

/// The minimal two's-complement form of `bytes` (X.690 8.3.2): leading bytes
/// that only repeat the sign are dropped. An empty slice is zero.
pub(crate) fn minimal(bytes: &[u8]) -> &[u8] {
    if bytes.is_empty() {
        return &[0];
    }
    let mut start = 0;
    while start + 1 < bytes.len() {
        let (first, next) = (bytes[start], bytes[start + 1]);
        let redundant = (first == 0x00 && next & 0x80 == 0) || (first == 0xFF && next & 0x80 != 0);
        if !redundant {
            break;
        }
        start += 1;
    }
    &bytes[start..]
}

/// A finite `f64` that is exactly an integer: `±mantissa × 2^shift`.
pub(crate) struct Integral {
    negative: bool,
    mantissa: u64,
    shift: u32,
}

impl Integral {
    /// `Some` if `x` is finite and exactly equal to an integer.
    pub(crate) fn of(x: f64) -> Option<Integral> {
        let bits = x.to_bits();
        let negative = bits >> 63 == 1;
        let exponent = ((bits >> 52) & 0x7FF) as i32;
        let fraction = bits & ((1 << 52) - 1);
        match exponent {
            0x7FF => None, // infinite or NaN
            0 if fraction == 0 => Some(Integral {
                negative: false,
                mantissa: 0,
                shift: 0,
            }),
            0 => None, // subnormal: strictly between -1 and 1, and not zero
            _ => {
                let mantissa = fraction | (1 << 52);
                let e = exponent - 1075; // x = ±mantissa × 2^e
                if e >= 0 {
                    return Some(Integral {
                        negative,
                        mantissa,
                        shift: e as u32,
                    });
                }
                let drop = (-e) as u32;
                if drop > 52 || mantissa & ((1 << drop) - 1) != 0 {
                    return None;
                }
                Some(Integral {
                    negative,
                    mantissa: mantissa >> drop,
                    shift: 0,
                })
            }
        }
    }

    /// The integer's minimal two's-complement bytes.
    pub(crate) fn twos_complement<'b>(&self, buf: &'b mut [u8; FLOAT_INT_BUF]) -> &'b [u8] {
        let (whole_bytes, bits) = ((self.shift / 8) as usize, self.shift % 8);
        let len = 1 + 8 + whole_bytes;
        let buf = &mut buf[..len];
        buf.fill(0);
        buf[1..9].copy_from_slice(&(self.mantissa << bits).to_be_bytes());
        if self.negative {
            negate(buf);
        }
        minimal(buf)
    }
}

/// Negates a big-endian two's-complement integer in place.
pub(crate) fn negate(bytes: &mut [u8]) {
    let mut carry = true;
    for b in bytes.iter_mut().rev() {
        let (sum, overflow) = (!*b).overflowing_add(carry as u8);
        *b = sum;
        carry = overflow;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn int(v: impl Integer) -> std::vec::Vec<u8> {
        v.twos_complement(&mut [0; INT_BUF]).to_vec()
    }

    fn float(x: f64) -> Option<std::vec::Vec<u8>> {
        Integral::of(x).map(|i| i.twos_complement(&mut [0; FLOAT_INT_BUF]).to_vec())
    }

    #[test]
    fn integers() {
        assert_eq!(int(0u8), [0x00]);
        assert_eq!(int(127i8), [0x7F]);
        assert_eq!(int(128u8), [0x00, 0x80]);
        assert_eq!(int(-128i8), [0x80]);
        assert_eq!(int(-129i16), [0xFF, 0x7F]);
        assert_eq!(int(u128::MAX), [[0x00].as_slice(), &[0xFF; 16]].concat());
        assert_eq!(int(i128::MIN), [[0x80].as_slice(), &[0x00; 15]].concat());
        assert_eq!(minimal(&[]), [0x00]);
        assert_eq!(minimal(&[0xFF, 0xFF, 0xFF]), [0xFF]);
    }

    #[test]
    fn integral_floats() {
        assert_eq!(float(0.0), Some(int(0)));
        assert_eq!(float(-0.0), Some(int(0)));
        assert_eq!(float(1.0), Some(int(1)));
        assert_eq!(float(-2.0), Some(int(-2)));
        assert_eq!(float(9007199254740992.0), Some(int(9007199254740992u64)));
        assert_eq!(float(18446744073709551616.0), Some(int(1u128 << 64)));
        assert_eq!(
            float(-170141183460469231731687303715884105728.0),
            Some(int(i128::MIN))
        );
        assert_eq!(float(f64::MAX).map(|b| b.len()), Some(129));
        assert_eq!(float(0.5), None);
        assert_eq!(float(1.5), None);
        assert_eq!(float(f64::MIN_POSITIVE), None);
        assert_eq!(float(5e-324), None);
        assert_eq!(float(f64::INFINITY), None);
    }
}
