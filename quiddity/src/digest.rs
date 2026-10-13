//! Digests and their text forms (spec section 4).

use core::fmt;
use core::str::FromStr;

use sha2::Digest as _;
use sha2::Sha256;

/// Prefix of the domain step's hash input (spec 4.2).
const DOMAIN_PREFIX: &[u8] = b"quiddity/1/domain";

macro_rules! digest_type {
    ($(#[$doc:meta])* $name:ident, $len:literal, $prefix:literal) => {
        $(#[$doc])*
        #[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
        pub struct $name([u8; $len]);

        impl $name {
            /// The text form's prefix, including the colon.
            pub const PREFIX: &'static str = $prefix;

            /// The digest with these bytes.
            pub const fn from_bytes(bytes: [u8; $len]) -> Self {
                $name(bytes)
            }

            /// The digest's bytes.
            pub const fn as_bytes(&self) -> &[u8; $len] {
                &self.0
            }
        }

        impl From<[u8; $len]> for $name {
            fn from(bytes: [u8; $len]) -> Self {
                $name(bytes)
            }
        }

        impl From<$name> for [u8; $len] {
            fn from(digest: $name) -> Self {
                digest.0
            }
        }

        impl AsRef<[u8]> for $name {
            fn as_ref(&self) -> &[u8] {
                &self.0
            }
        }

        /// The text form, e.g. `sha256:9f2c…`, with lowercase hex.
        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str($prefix)?;
                for b in &self.0 {
                    write!(f, "{b:02x}")?;
                }
                Ok(())
            }
        }

        impl fmt::Debug for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}(\"{}\")", stringify!($name), self)
            }
        }

        /// Parses the text form. Hex digits may be upper or lower case; the
        /// prefix must be lowercase and must match this digest's size.
        impl FromStr for $name {
            type Err = ParseDigestError;

            fn from_str(s: &str) -> Result<Self, ParseDigestError> {
                parse_hex(s, $prefix).map($name)
            }
        }
    };
}

digest_type! {
    /// A 256-bit digest: a SHA-256 hash. Written as text `sha256:` followed by
    /// 64 hex digits.
    Digest, 32, "sha256:"
}

digest_type! {
    /// The first 128 bits of a [`Digest`], from [`Digest::mini`]. Written as
    /// text `sha256-128:` followed by 32 hex digits.
    Digest128, 16, "sha256-128:"
}

digest_type! {
    /// The first 64 bits of a [`Digest`], from [`Digest::micro`]. Written as
    /// text `sha256-64:` followed by 16 hex digits.
    Digest64, 8, "sha256-64:"
}

impl Digest {
    /// This digest raised into the domain `label` (spec 4.2). For a
    /// structural digest `s` of a value, `s.in_domain(label)` is the digest
    /// of that value in a type whose domain is `label`.
    ///
    /// # Panics
    ///
    /// If `label` is empty.
    pub fn in_domain(&self, label: &str) -> Digest {
        assert!(
            !label.is_empty(),
            "quiddity: a domain label must not be empty"
        );
        let mut hash = Sha256::new();
        hash.update(DOMAIN_PREFIX);
        hash.update(label.as_bytes());
        hash.update((label.len() as u64).to_be_bytes());
        hash.update(self.0);
        Digest(hash.finalize().into())
    }

    /// The first 128 bits of this digest.
    pub fn mini(&self) -> Digest128 {
        let mut bytes = [0; 16];
        bytes.copy_from_slice(&self.0[..16]);
        Digest128(bytes)
    }

    /// The first 64 bits of this digest.
    pub fn micro(&self) -> Digest64 {
        let mut bytes = [0; 8];
        bytes.copy_from_slice(&self.0[..8]);
        Digest64(bytes)
    }
}

/// The error returned when a digest's text form cannot be parsed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParseDigestError {
    expected_prefix: &'static str,
    expected_digits: usize,
}

impl fmt::Display for ParseDigestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "expected `{}` followed by {} hex digits",
            self.expected_prefix, self.expected_digits
        )
    }
}

impl core::error::Error for ParseDigestError {}

fn parse_hex<const N: usize>(s: &str, prefix: &'static str) -> Result<[u8; N], ParseDigestError> {
    let error = ParseDigestError {
        expected_prefix: prefix,
        expected_digits: 2 * N,
    };
    let hex = s.strip_prefix(prefix).ok_or(error.clone())?.as_bytes();
    if hex.len() != 2 * N {
        return Err(error);
    }
    let mut bytes = [0; N];
    for (byte, pair) in bytes.iter_mut().zip(hex.chunks_exact(2)) {
        let digit = |c: u8| (c as char).to_digit(16).ok_or(error.clone());
        *byte = (digit(pair[0])? << 4 | digit(pair[1])?) as u8;
    }
    Ok(bytes)
}
