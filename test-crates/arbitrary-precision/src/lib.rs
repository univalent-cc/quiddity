//! With serde_json's `arbitrary_precision` feature, a `Number` keeps its
//! decimal text. Its digest must be the one it would have without the
//! feature (spec 5.6): an integer within `i64` or `u64` is that integer, and
//! anything else is rounded to the nearest `f64`, then normalized.

#[cfg(test)]
mod tests {
    use quiddity::{Digest, QuiddityExt};
    use serde_json::Value;

    fn digest(json: &str) -> Digest {
        serde_json::from_str::<Value>(json).unwrap().quiddity()
    }

    #[test]
    fn feature_is_on() {
        // Without the feature, serde_json rejects numbers beyond `f64`.
        assert!(serde_json::from_str::<Value>("1e400").is_ok());
    }

    #[test]
    fn integers() {
        assert_eq!(digest("1"), 1u8.quiddity());
        assert_eq!(digest("-1"), (-1i8).quiddity());
        assert_eq!(digest("9223372036854775807"), i64::MAX.quiddity());
        assert_eq!(digest("-9223372036854775808"), i64::MIN.quiddity());
        assert_eq!(digest("18446744073709551615"), u64::MAX.quiddity());
    }

    #[test]
    fn integral_floats_are_integers() {
        assert_eq!(digest("1.0"), 1u8.quiddity());
        assert_eq!(digest("1e2"), 100u8.quiddity());
        assert_eq!(digest("-0"), 0u8.quiddity());
        assert_eq!(digest("-0.0"), 0u8.quiddity());
    }

    #[test]
    fn beyond_64_bits_rounds_to_f64() {
        let two_64 = 1u128 << 64;
        assert_eq!(digest("18446744073709551616"), two_64.quiddity());
        // 2^64 + 1 is not an `f64`; it rounds to 2^64, as it does without
        // the feature.
        assert_eq!(digest("18446744073709551617"), two_64.quiddity());
        assert_eq!(digest("-9223372036854775809"), (-(1i128 << 63)).quiddity());
    }

    #[test]
    fn decimals_round_to_f64() {
        assert_eq!(digest("1.5"), 1.5f64.quiddity());
        assert_eq!(digest("0.1"), 0.1f64.quiddity());
        assert_eq!(digest("0.10000000000000000001"), 0.1f64.quiddity());
        assert_eq!(digest("1e-400"), 0u8.quiddity());
    }

    #[test]
    fn beyond_f64_is_infinite() {
        assert_eq!(digest("1e400"), f64::INFINITY.quiddity());
        assert_eq!(digest("-1e400"), f64::NEG_INFINITY.quiddity());
    }
}
