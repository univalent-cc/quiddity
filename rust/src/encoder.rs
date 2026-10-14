//! The `Encoder`: writes values in the encoding of `QUIDDITY_SPEC.md`
//! section 3, straight into a SHA-256 state.

#[cfg(feature = "alloc")]
use alloc::vec::Vec;
use core::cmp::Ordering;
use core::fmt;

use sha2::Digest as _;
use sha2::Sha256;

use crate::digest::Digest;
use crate::number::{self, Integer};
use crate::{IdentityMarker, Primary, Quiddity};

/// Kind tags (spec 3.2).
pub(crate) mod tag {
    pub const NULL: u8 = 0x01;
    pub const FALSE: u8 = 0x02;
    pub const TRUE: u8 = 0x03;
    pub const INT: u8 = 0x10;
    pub const FLOAT: u8 = 0x11;
    pub const STRING: u8 = 0x20;
    pub const LIST: u8 = 0x30;
    #[cfg(feature = "alloc")]
    pub const SET: u8 = 0x31;
    pub const MAP: u8 = 0x32;
}

/// Prefix of the structural digest's hash input (spec 4.1).
const VALUE_PREFIX: &[u8] = b"quiddity/1/value";

/// Writes a value's encoding.
///
/// An `Encoder` is handed to [`Quiddity::encode`] and to `with` functions.
/// It writes one value: a primitive (`null`, `bool`, `int`, `float`, and a
/// string with `str` or `bytes`), or a composite (`list`, `set`, `map`, `fields`) whose writer
/// encodes the items.
pub struct Encoder {
    sink: Sink,
}

enum Sink {
    Hash(Sha256),
    /// Used to encode set members and map entries, which are sorted by
    /// their encodings before being written (spec 2.5).
    #[cfg(feature = "alloc")]
    Buffer(Vec<u8>),
}

impl Encoder {
    /// An encoder for a structural digest (spec 4.1).
    pub(crate) fn new() -> Self {
        let mut hash = Sha256::new();
        hash.update(VALUE_PREFIX);
        Encoder {
            sink: Sink::Hash(hash),
        }
    }

    /// The structural digest of what was written.
    pub(crate) fn finish(self) -> Digest {
        match self.sink {
            Sink::Hash(hash) => Digest::from_bytes(hash.finalize().into()),
            #[cfg(feature = "alloc")]
            Sink::Buffer(_) => unreachable!("finish called on a buffering encoder"),
        }
    }

    #[cfg(feature = "alloc")]
    pub(crate) fn buffer(buf: Vec<u8>) -> Self {
        Encoder {
            sink: Sink::Buffer(buf),
        }
    }

    #[cfg(feature = "alloc")]
    pub(crate) fn into_buffer(self) -> Vec<u8> {
        match self.sink {
            Sink::Buffer(buf) => buf,
            Sink::Hash(_) => unreachable!("into_buffer called on a hashing encoder"),
        }
    }

    pub(crate) fn write(&mut self, bytes: &[u8]) {
        match &mut self.sink {
            Sink::Hash(hash) => hash.update(bytes),
            #[cfg(feature = "alloc")]
            Sink::Buffer(buf) => buf.extend_from_slice(bytes),
        }
    }

    pub(crate) fn write_len_tag(&mut self, len: usize, tag: u8) {
        self.write(&(len as u64).to_be_bytes());
        self.write(&[tag]);
    }

    /// Writes null.
    pub fn null(&mut self) {
        self.write(&[tag::NULL]);
    }

    /// Writes a boolean.
    pub fn bool(&mut self, value: bool) {
        self.write(&[if value { tag::TRUE } else { tag::FALSE }]);
    }

    /// Writes an integer of any primitive integer type.
    pub fn int(&mut self, value: impl Integer) {
        let mut buf = [0u8; number::INT_BUF];
        let content = value.twos_complement(&mut buf);
        self.int_twos_complement(content);
    }

    /// Writes an integer given as big-endian two's-complement bytes, of any
    /// length. Redundant leading sign bytes are removed, and an empty slice
    /// is zero. This is the form `num_bigint::BigInt::to_signed_bytes_be`
    /// produces.
    pub fn int_twos_complement(&mut self, bytes: &[u8]) {
        let content = number::minimal(bytes);
        self.write(content);
        self.write_len_tag(content.len(), tag::INT);
    }

    /// Writes a number given as an `f64`. A value exactly equal to an
    /// integer is written as that integer, and every NaN as the same NaN
    /// (spec 2.2). An `f32` is widened with `f64::from`.
    pub fn float(&mut self, value: f64) {
        if value.is_nan() {
            self.write(&number::CANONICAL_NAN.to_be_bytes());
            self.write(&[tag::FLOAT]);
        } else if let Some(int) = number::Integral::of(value) {
            let mut buf = [0u8; number::FLOAT_INT_BUF];
            self.int_twos_complement(int.twos_complement(&mut buf));
        } else {
            self.write(&value.to_bits().to_be_bytes());
            self.write(&[tag::FLOAT]);
        }
    }

    /// Writes a string of text, as its UTF-8 bytes.
    pub fn str(&mut self, value: &str) {
        self.bytes(value.as_bytes());
    }

    /// Writes a string of bytes. Text and bytes are the same kind (spec
    /// 2.3): `bytes(b"abc")` writes the same as `str("abc")`. (`Vec<u8>` and
    /// `[u8]` as values are lists of integers; this is the way to write
    /// binary data as a string.)
    pub fn bytes(&mut self, value: &[u8]) {
        self.write(value);
        self.write_len_tag(value.len(), tag::STRING);
    }

    /// Writes a string supplied in pieces, through [`core::fmt::Write`].
    /// The digest is the same as for the whole string written with
    /// [`str`](Self::str).
    pub fn str_stream(&mut self) -> StrStream<'_> {
        StrStream { e: self, len: 0 }
    }

    /// Writes a string of bytes supplied in pieces. The digest is the same
    /// as for the whole string written with [`bytes`](Self::bytes).
    pub fn bytes_stream(&mut self) -> BytesStream<'_> {
        BytesStream { e: self, len: 0 }
    }

    /// Writes text given as UTF-16 code units, which may be ill-formed, as
    /// its WTF-8 bytes (spec 2.3).
    #[cfg(any(all(windows, feature = "std"), all(test, feature = "alloc")))]
    pub(crate) fn utf16(&mut self, units: impl IntoIterator<Item = u16>) {
        let mut s = self.bytes_stream();
        for unit in char::decode_utf16(units) {
            match unit {
                Ok(c) => s.push(c.encode_utf8(&mut [0; 4]).as_bytes()),
                Err(err) => {
                    // The three bytes UTF-8's scheme gives the code point.
                    let u = err.unpaired_surrogate();
                    s.push(&[
                        0xE0 | (u >> 12) as u8,
                        0x80 | ((u >> 6) & 0x3F) as u8,
                        0x80 | (u & 0x3F) as u8,
                    ]);
                }
            }
        }
    }

    /// Writes the string produced by `value`'s `Display` impl.
    ///
    /// # Panics
    ///
    /// If the `Display` impl returns an error, as `ToString` does.
    pub fn display<T: fmt::Display + ?Sized>(&mut self, value: &T) {
        let mut s = self.str_stream();
        fmt::Write::write_fmt(&mut s, format_args!("{value}"))
            .expect("a Display implementation returned an error unexpectedly");
        s.finish();
    }

    /// Writes a list. Items are written through the returned writer, in
    /// order.
    pub fn list(&mut self) -> ListEncoder<'_> {
        ListEncoder { e: self, count: 0 }
    }

    /// Writes a set. Members may be written in any order; they are sorted
    /// (spec 2.5).
    #[cfg(feature = "alloc")]
    pub fn set(&mut self) -> SetEncoder<'_> {
        SetEncoder {
            e: self,
            buf: Vec::new(),
            spans: Vec::new(),
        }
    }

    /// Writes a map. Entries may be written in any order; they are sorted
    /// (spec 2.5).
    #[cfg(feature = "alloc")]
    pub fn map(&mut self) -> MapEncoder<'_> {
        MapEncoder {
            e: self,
            entries: Entries::default(),
        }
    }

    /// Writes a map whose keys are strings known in advance, such as a
    /// struct's field names, without buffering. Fields must be written in
    /// canonical order, which for names without U+0000 is code-point order;
    /// debug builds check it.
    pub fn fields(&mut self) -> FieldsEncoder<'_> {
        FieldsEncoder {
            e: self,
            count: 0,
            #[cfg(debug_assertions)]
            last: None,
        }
    }
}

/// Writes a string in pieces. Created by [`Encoder::str_stream`]; the
/// string ends when this is finished or dropped.
pub struct StrStream<'e> {
    e: &'e mut Encoder,
    len: usize,
}

impl StrStream<'_> {
    /// Appends `s` to the string.
    pub fn push(&mut self, s: &str) {
        self.e.write(s.as_bytes());
        self.len += s.len();
    }

    /// Ends the string. Dropping the writer does the same.
    pub fn finish(self) {}
}

impl fmt::Write for StrStream<'_> {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.push(s);
        Ok(())
    }
}

impl Drop for StrStream<'_> {
    fn drop(&mut self) {
        self.e.write_len_tag(self.len, tag::STRING);
    }
}

/// Writes a string of bytes in pieces. Created by [`Encoder::bytes_stream`];
/// the string ends when this is finished or dropped.
pub struct BytesStream<'e> {
    e: &'e mut Encoder,
    len: usize,
}

impl BytesStream<'_> {
    /// Appends `bytes` to the string.
    pub fn push(&mut self, bytes: &[u8]) {
        self.e.write(bytes);
        self.len += bytes.len();
    }

    /// Ends the string. Dropping the writer does the same.
    pub fn finish(self) {}
}

#[cfg(feature = "std")]
impl std::io::Write for BytesStream<'_> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.push(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl Drop for BytesStream<'_> {
    fn drop(&mut self) {
        self.e.write_len_tag(self.len, tag::STRING);
    }
}

/// Writes the items of a list. Created by [`Encoder::list`]; the list ends
/// when this is finished or dropped.
pub struct ListEncoder<'e> {
    e: &'e mut Encoder,
    count: usize,
}

impl ListEncoder<'_> {
    /// Appends `item`, under its primary identity.
    pub fn item<T: Quiddity + ?Sized>(&mut self, item: &T) {
        self.item_as::<Primary, T>(item);
    }

    /// Appends `item`, under identity `I`.
    pub fn item_as<I: IdentityMarker, T: Quiddity<I> + ?Sized>(&mut self, item: &T) {
        self.item_with(|e| <T as Quiddity<I>>::encode(item, e));
    }

    /// Appends the one value that `f` writes.
    pub fn item_with(&mut self, f: impl FnOnce(&mut Encoder)) {
        f(self.e);
        self.count += 1;
    }

    /// Ends the list. Dropping the writer does the same.
    pub fn finish(self) {}
}

impl Drop for ListEncoder<'_> {
    fn drop(&mut self) {
        self.e.write_len_tag(self.count, tag::LIST);
    }
}

/// Writes the members of a set. Created by [`Encoder::set`]; the set ends
/// when this is finished or dropped.
#[cfg(feature = "alloc")]
pub struct SetEncoder<'e> {
    e: &'e mut Encoder,
    /// The members' encodings, one after another.
    buf: Vec<u8>,
    /// Each member's range in `buf`.
    spans: Vec<(usize, usize)>,
}

#[cfg(feature = "alloc")]
impl SetEncoder<'_> {
    /// Adds `member`, under its primary identity.
    pub fn member<T: Quiddity + ?Sized>(&mut self, member: &T) {
        self.member_as::<Primary, T>(member);
    }

    /// Adds `member`, under identity `I`.
    pub fn member_as<I: IdentityMarker, T: Quiddity<I> + ?Sized>(&mut self, member: &T) {
        self.member_with(|e| <T as Quiddity<I>>::encode(member, e));
    }

    /// Adds the one value that `f` writes.
    pub fn member_with(&mut self, f: impl FnOnce(&mut Encoder)) {
        let start = self.buf.len();
        self.buf = encode_into(core::mem::take(&mut self.buf), f);
        self.spans.push((start, self.buf.len()));
    }

    /// Ends the set. Dropping the writer does the same.
    pub fn finish(self) {}
}

#[cfg(feature = "alloc")]
impl Drop for SetEncoder<'_> {
    fn drop(&mut self) {
        let buf = &self.buf;
        self.spans
            .sort_unstable_by(|a, b| buf[a.0..a.1].cmp(&buf[b.0..b.1]));
        for &(start, end) in &self.spans {
            self.e.write(&buf[start..end]);
        }
        self.e.write_len_tag(self.spans.len(), tag::SET);
    }
}

/// Writes the entries of a map. Created by [`Encoder::map`]; the map ends
/// when this is finished or dropped.
#[cfg(feature = "alloc")]
pub struct MapEncoder<'e> {
    e: &'e mut Encoder,
    entries: Entries,
}

#[cfg(feature = "alloc")]
impl MapEncoder<'_> {
    /// Adds an entry, with key and value under their primary identities.
    pub fn entry<K, V>(&mut self, key: &K, value: &V)
    where
        K: Quiddity + ?Sized,
        V: Quiddity + ?Sized,
    {
        self.entry_as::<Primary, K, V>(key, value);
    }

    /// Adds an entry, with key and value under identity `I`.
    pub fn entry_as<I, K, V>(&mut self, key: &K, value: &V)
    where
        I: IdentityMarker,
        K: Quiddity<I> + ?Sized,
        V: Quiddity<I> + ?Sized,
    {
        self.entry_with(
            |e| <K as Quiddity<I>>::encode(key, e),
            |e| <V as Quiddity<I>>::encode(value, e),
        );
    }

    /// Adds an entry whose key is the one value `key` writes, and whose
    /// value is the one value `value` writes.
    pub fn entry_with(&mut self, key: impl FnOnce(&mut Encoder), value: impl FnOnce(&mut Encoder)) {
        self.entries.key_with(key);
        self.entries.value_with(value);
    }

    /// Ends the map. Dropping the writer does the same.
    pub fn finish(self) {}
}

#[cfg(feature = "alloc")]
impl Drop for MapEncoder<'_> {
    fn drop(&mut self) {
        core::mem::take(&mut self.entries).write_sorted(self.e);
    }
}

/// A map's entries, buffered for sorting. Keys and values can be added in
/// separate steps, as serde supplies them.
#[cfg(feature = "alloc")]
#[derive(Default)]
pub(crate) struct Entries {
    /// The entries' encodings, key then value, one after another.
    buf: Vec<u8>,
    /// Each entry's start, end of key, and end of value in `buf`.
    spans: Vec<(usize, usize, usize)>,
    /// The start and end of a key whose value has not been added yet.
    pending: Option<(usize, usize)>,
}

#[cfg(feature = "alloc")]
impl Entries {
    /// Adds the key of a new entry: the one value `f` writes.
    pub(crate) fn key_with<R>(&mut self, f: impl FnOnce(&mut Encoder) -> R) -> R {
        assert!(
            self.pending.is_none(),
            "quiddity: a map key was added without its value"
        );
        let start = self.buf.len();
        let (buf, r) = encode_into_with(core::mem::take(&mut self.buf), f);
        self.buf = buf;
        self.pending = Some((start, self.buf.len()));
        r
    }

    /// Adds the value of the entry whose key was just added.
    pub(crate) fn value_with<R>(&mut self, f: impl FnOnce(&mut Encoder) -> R) -> R {
        let (start, mid) = self
            .pending
            .take()
            .expect("quiddity: a map value was added without a key");
        let (buf, r) = encode_into_with(core::mem::take(&mut self.buf), f);
        self.buf = buf;
        self.spans.push((start, mid, self.buf.len()));
        r
    }

    /// Writes the map: its entries in canonical order (spec 2.5), then the
    /// count and tag.
    pub(crate) fn write_sorted(mut self, e: &mut Encoder) {
        let buf = &self.buf;
        self.spans.sort_unstable_by(|a, b| {
            (&buf[a.0..a.1], &buf[a.1..a.2]).cmp(&(&buf[b.0..b.1], &buf[b.1..b.2]))
        });
        for &(start, _, end) in &self.spans {
            e.write(&buf[start..end]);
        }
        e.write_len_tag(self.spans.len(), tag::MAP);
    }
}

/// Appends what `f` writes to `buf`.
#[cfg(feature = "alloc")]
fn encode_into(buf: Vec<u8>, f: impl FnOnce(&mut Encoder)) -> Vec<u8> {
    encode_into_with(buf, f).0
}

/// Appends what `f` writes to `buf`, and returns `f`'s result too.
#[cfg(feature = "alloc")]
fn encode_into_with<R>(buf: Vec<u8>, f: impl FnOnce(&mut Encoder) -> R) -> (Vec<u8>, R) {
    let mut e = Encoder::buffer(buf);
    let r = f(&mut e);
    (e.into_buffer(), r)
}

/// Writes the fields of a map with string keys known in advance. Created by
/// [`Encoder::fields`]; the map ends when this is finished or dropped.
pub struct FieldsEncoder<'e> {
    e: &'e mut Encoder,
    count: usize,
    #[cfg(debug_assertions)]
    last: Option<&'static str>,
}

impl FieldsEncoder<'_> {
    /// Adds the field `name`, with `value` under its primary identity.
    pub fn field<T: Quiddity + ?Sized>(&mut self, name: &'static str, value: &T) {
        self.field_as::<Primary, T>(name, value);
    }

    /// Adds the field `name`, with `value` under identity `I`.
    pub fn field_as<I: IdentityMarker, T: Quiddity<I> + ?Sized>(
        &mut self,
        name: &'static str,
        value: &T,
    ) {
        self.field_with(name, |e| <T as Quiddity<I>>::encode(value, e));
    }

    /// Adds the field `name`, whose value is the one value `f` writes.
    pub fn field_with(&mut self, name: &'static str, f: impl FnOnce(&mut Encoder)) {
        #[cfg(debug_assertions)]
        {
            if let Some(last) = self.last {
                assert!(
                    cmp_key_encodings(last, name) == Ordering::Less,
                    "quiddity: field {name:?} written after {last:?}; \
                     fields must be written in canonical order, without repeats",
                );
            }
            self.last = Some(name);
        }
        self.e.str(name);
        f(self.e);
        self.count += 1;
    }

    /// Ends the map. Dropping the writer does the same.
    pub fn finish(self) {}
}

impl Drop for FieldsEncoder<'_> {
    fn drop(&mut self) {
        self.e.write_len_tag(self.count, tag::MAP);
    }
}

/// Compares the encodings of two strings (spec 2.5), without building them.
pub(crate) fn cmp_key_encodings(a: &str, b: &str) -> Ordering {
    fn encoding(s: &str) -> impl Iterator<Item = u8> + '_ {
        let len = (s.len() as u64).to_be_bytes();
        s.bytes().chain(len).chain([tag::STRING])
    }
    encoding(a).cmp(encoding(b))
}

#[cfg(test)]
mod tests {
    use super::{Encoder, cmp_key_encodings};
    use core::cmp::Ordering::*;
    #[cfg(feature = "alloc")]
    use std::vec::Vec;

    #[cfg(feature = "alloc")]
    fn encoding(f: impl FnOnce(&mut Encoder)) -> Vec<u8> {
        super::encode_into(Vec::new(), f)
    }

    #[cfg(feature = "alloc")]
    #[test]
    fn fields_equal_map() {
        let fields = encoding(|e| {
            let mut f = e.fields();
            f.field_with("end", |e| e.int(15));
            f.field_with("start", |e| e.int(10));
        });
        let map = encoding(|e| {
            let mut m = e.map();
            m.entry_with(|e| e.str("start"), |e| e.int(10));
            m.entry_with(|e| e.str("end"), |e| e.int(15));
        });
        assert_eq!(fields, map);
    }

    #[cfg(feature = "alloc")]
    #[test]
    fn streams_equal_whole() {
        assert_eq!(
            encoding(|e| {
                let mut s = e.str_stream();
                s.push("hello, ");
                s.push("world");
            }),
            encoding(|e| e.str("hello, world")),
        );
        assert_eq!(
            encoding(|e| {
                let mut s = e.bytes_stream();
                s.push(&[1, 2]);
                s.push(&[3]);
            }),
            encoding(|e| e.bytes(&[1, 2, 3])),
        );
        assert_eq!(encoding(|e| e.display(&42u8)), encoding(|e| e.str("42")));
        assert_eq!(encoding(|e| e.bytes(b"abc")), encoding(|e| e.str("abc")));
    }

    #[cfg(feature = "alloc")]
    #[test]
    fn numbers_by_value() {
        assert_eq!(encoding(|e| e.float(1.0)), encoding(|e| e.int(1u8)));
        assert_eq!(encoding(|e| e.float(-0.0)), encoding(|e| e.int(0i64)));
        assert_eq!(encoding(|e| e.int(-1i8)), encoding(|e| e.int(-1i128)));
        assert_eq!(
            encoding(|e| e.int(255u8)),
            encoding(|e| e.int_twos_complement(&[0, 0, 0xFF]))
        );
        assert_eq!(
            encoding(|e| e.float(f64::from(0.5f32))),
            encoding(|e| e.float(0.5))
        );
    }

    #[cfg(debug_assertions)]
    #[test]
    #[should_panic(expected = "canonical order")]
    fn fields_out_of_order() {
        let mut e = Encoder::new();
        let mut f = e.fields();
        f.field_with("start", |e| e.null());
        f.field_with("end", |e| e.null());
    }

    #[test]
    fn key_order() {
        assert_eq!(cmp_key_encodings("end", "start"), Less);
        assert_eq!(cmp_key_encodings("a", "ab"), Less);
        assert_eq!(cmp_key_encodings("aa", "b"), Less);
        assert_eq!(cmp_key_encodings("a\0", "a"), Less);
        assert_eq!(cmp_key_encodings("a", "a"), Equal);
    }
}
