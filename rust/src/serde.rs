//! Digests of any `Serialize` value (spec 6.7), with the `serde` feature.
//!
//! A serializer that writes serde's data model into an [`Encoder`], in the
//! shapes serde gives values: so a `Serialize` value has the same digest as
//! the equivalent `Quiddity` value.

use alloc::string::{String, ToString};
use core::fmt;

use ::serde::ser::{self, Serialize};

use crate::encoder::{Entries, tag};
use crate::{Digest, Encoder, ListEncoder};

/// The structural digest of `value`, through its `Serialize` impl.
///
/// # Errors
///
/// Returns the error `value`'s `Serialize` impl returns, if any.
pub fn digest<T: Serialize + ?Sized>(value: &T) -> Result<Digest, Error> {
    let mut e = Encoder::new();
    encode(value, &mut e)?;
    Ok(e.finish())
}

/// Writes `value` through its `Serialize` impl, for manual `Quiddity` impls
/// and `with` functions.
///
/// # Errors
///
/// Returns the error `value`'s `Serialize` impl returns, if any. The
/// encoder's contents are then unspecified.
pub fn encode<T: Serialize + ?Sized>(value: &T, e: &mut Encoder) -> Result<(), Error> {
    value.serialize(Serializer { e })
}

/// [`encode`], panicking on error: for `encode_as = serde`, inside the
/// infallible `quiddity()`.
#[doc(hidden)]
pub fn encode_or_panic<T: Serialize + ?Sized>(value: &T, e: &mut Encoder) {
    if let Err(err) = encode(value, e) {
        panic!("quiddity: a `Serialize` impl failed while computing a digest: {err}");
    }
}

/// An error from a `Serialize` impl.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Error(String);

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl core::error::Error for Error {}

impl ser::Error for Error {
    fn custom<T: fmt::Display>(msg: T) -> Self {
        Error(msg.to_string())
    }
}

/// The serializer that writes into an [`Encoder`]. Hidden: used by the
/// derive for `serialize_with` (D21).
#[doc(hidden)]
pub struct Serializer<'a> {
    e: &'a mut Encoder,
}

/// Writes a value through `f`, a serde `serialize_with` function applied to
/// the bridge's serializer, panicking on error: for the derive's
/// `serde_attributes` mode, inside the infallible `quiddity()`.
#[doc(hidden)]
pub fn encode_with_or_panic(e: &mut Encoder, f: impl FnOnce(Serializer<'_>) -> Result<(), Error>) {
    if let Err(err) = f(Serializer { e }) {
        panic!("quiddity: a `serialize_with` function failed while computing a digest: {err}");
    }
}

/// Writes `value` through its `Serialize` impl.
fn serialize_into<T: Serialize + ?Sized>(value: &T, e: &mut Encoder) -> Result<(), Error> {
    value.serialize(Serializer { e })
}

/// Writes the key of a single-entry map `{variant: …}`; the value follows,
/// then [`end_variant`].
fn begin_variant(e: &mut Encoder, variant: &str) {
    e.str(variant);
}

fn end_variant(e: &mut Encoder) {
    e.write_len_tag(1, tag::MAP);
}

impl<'a> ser::Serializer for Serializer<'a> {
    type Ok = ();
    type Error = Error;
    type SerializeSeq = Seq<'a>;
    type SerializeTuple = Seq<'a>;
    type SerializeTupleStruct = Seq<'a>;
    type SerializeTupleVariant = VariantSeq<'a>;
    type SerializeMap = Map<'a>;
    type SerializeStruct = Map<'a>;
    type SerializeStructVariant = Map<'a>;

    fn is_human_readable(&self) -> bool {
        true
    }

    fn serialize_bool(self, v: bool) -> Result<(), Error> {
        self.e.bool(v);
        Ok(())
    }

    fn serialize_i8(self, v: i8) -> Result<(), Error> {
        self.e.int(v);
        Ok(())
    }

    fn serialize_i16(self, v: i16) -> Result<(), Error> {
        self.e.int(v);
        Ok(())
    }

    fn serialize_i32(self, v: i32) -> Result<(), Error> {
        self.e.int(v);
        Ok(())
    }

    fn serialize_i64(self, v: i64) -> Result<(), Error> {
        self.e.int(v);
        Ok(())
    }

    fn serialize_i128(self, v: i128) -> Result<(), Error> {
        self.e.int(v);
        Ok(())
    }

    fn serialize_u8(self, v: u8) -> Result<(), Error> {
        self.e.int(v);
        Ok(())
    }

    fn serialize_u16(self, v: u16) -> Result<(), Error> {
        self.e.int(v);
        Ok(())
    }

    fn serialize_u32(self, v: u32) -> Result<(), Error> {
        self.e.int(v);
        Ok(())
    }

    fn serialize_u64(self, v: u64) -> Result<(), Error> {
        self.e.int(v);
        Ok(())
    }

    fn serialize_u128(self, v: u128) -> Result<(), Error> {
        self.e.int(v);
        Ok(())
    }

    fn serialize_f32(self, v: f32) -> Result<(), Error> {
        self.e.float(f64::from(v));
        Ok(())
    }

    fn serialize_f64(self, v: f64) -> Result<(), Error> {
        self.e.float(v);
        Ok(())
    }

    fn serialize_char(self, v: char) -> Result<(), Error> {
        self.e.str(v.encode_utf8(&mut [0; 4]));
        Ok(())
    }

    fn serialize_str(self, v: &str) -> Result<(), Error> {
        self.e.str(v);
        Ok(())
    }

    fn serialize_bytes(self, v: &[u8]) -> Result<(), Error> {
        self.e.bytes(v);
        Ok(())
    }

    fn serialize_none(self) -> Result<(), Error> {
        self.e.null();
        Ok(())
    }

    fn serialize_some<T: Serialize + ?Sized>(self, value: &T) -> Result<(), Error> {
        value.serialize(self)
    }

    fn serialize_unit(self) -> Result<(), Error> {
        self.e.null();
        Ok(())
    }

    fn serialize_unit_struct(self, _name: &'static str) -> Result<(), Error> {
        self.e.null();
        Ok(())
    }

    fn serialize_unit_variant(
        self,
        _name: &'static str,
        _index: u32,
        variant: &'static str,
    ) -> Result<(), Error> {
        self.e.str(variant);
        Ok(())
    }

    fn serialize_newtype_struct<T: Serialize + ?Sized>(
        self,
        _name: &'static str,
        value: &T,
    ) -> Result<(), Error> {
        value.serialize(self)
    }

    fn serialize_newtype_variant<T: Serialize + ?Sized>(
        self,
        _name: &'static str,
        _index: u32,
        variant: &'static str,
        value: &T,
    ) -> Result<(), Error> {
        begin_variant(self.e, variant);
        serialize_into(value, self.e)?;
        end_variant(self.e);
        Ok(())
    }

    fn serialize_seq(self, _len: Option<usize>) -> Result<Seq<'a>, Error> {
        Ok(Seq {
            list: self.e.list(),
        })
    }

    fn serialize_tuple(self, _len: usize) -> Result<Seq<'a>, Error> {
        Ok(Seq {
            list: self.e.list(),
        })
    }

    fn serialize_tuple_struct(self, _name: &'static str, _len: usize) -> Result<Seq<'a>, Error> {
        Ok(Seq {
            list: self.e.list(),
        })
    }

    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        _index: u32,
        variant: &'static str,
        _len: usize,
    ) -> Result<VariantSeq<'a>, Error> {
        begin_variant(self.e, variant);
        Ok(VariantSeq {
            e: self.e,
            count: 0,
        })
    }

    fn serialize_map(self, _len: Option<usize>) -> Result<Map<'a>, Error> {
        Ok(Map {
            e: self.e,
            entries: Entries::default(),
            variant: false,
        })
    }

    fn serialize_struct(self, _name: &'static str, _len: usize) -> Result<Map<'a>, Error> {
        Ok(Map {
            e: self.e,
            entries: Entries::default(),
            variant: false,
        })
    }

    fn serialize_struct_variant(
        self,
        _name: &'static str,
        _index: u32,
        variant: &'static str,
        _len: usize,
    ) -> Result<Map<'a>, Error> {
        begin_variant(self.e, variant);
        Ok(Map {
            e: self.e,
            entries: Entries::default(),
            variant: true,
        })
    }

    fn collect_str<T: fmt::Display + ?Sized>(self, value: &T) -> Result<(), Error> {
        self.e.display(value);
        Ok(())
    }
}

/// A sequence, tuple or tuple struct: a list.
#[doc(hidden)]
pub struct Seq<'a> {
    list: ListEncoder<'a>,
}

impl Seq<'_> {
    fn element<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), Error> {
        let mut result = Ok(());
        self.list.item_with(|e| result = serialize_into(value, e));
        result
    }
}

impl ser::SerializeSeq for Seq<'_> {
    type Ok = ();
    type Error = Error;

    fn serialize_element<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), Error> {
        self.element(value)
    }

    fn end(self) -> Result<(), Error> {
        Ok(())
    }
}

impl ser::SerializeTuple for Seq<'_> {
    type Ok = ();
    type Error = Error;

    fn serialize_element<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), Error> {
        self.element(value)
    }

    fn end(self) -> Result<(), Error> {
        Ok(())
    }
}

impl ser::SerializeTupleStruct for Seq<'_> {
    type Ok = ();
    type Error = Error;

    fn serialize_field<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), Error> {
        self.element(value)
    }

    fn end(self) -> Result<(), Error> {
        Ok(())
    }
}

/// A tuple variant: `{variant: [values]}`, after `begin_variant`. The
/// elements are written as they come, then the list's and the map's counts.
#[doc(hidden)]
pub struct VariantSeq<'a> {
    e: &'a mut Encoder,
    count: usize,
}

impl ser::SerializeTupleVariant for VariantSeq<'_> {
    type Ok = ();
    type Error = Error;

    fn serialize_field<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), Error> {
        serialize_into(value, self.e)?;
        self.count += 1;
        Ok(())
    }

    fn end(self) -> Result<(), Error> {
        self.e.write_len_tag(self.count, tag::LIST);
        end_variant(self.e);
        Ok(())
    }
}

/// A map, struct or struct variant: entries sorted when it ends. For a
/// struct variant, `{variant: {fields}}` after `begin_variant`.
#[doc(hidden)]
pub struct Map<'a> {
    e: &'a mut Encoder,
    entries: Entries,
    variant: bool,
}

impl Map<'_> {
    fn field<T: Serialize + ?Sized>(&mut self, key: &'static str, value: &T) -> Result<(), Error> {
        self.entries.key_with(|e| e.str(key));
        self.entries.value_with(|e| serialize_into(value, e))
    }

    fn finish(self) -> Result<(), Error> {
        self.entries.write_sorted(self.e);
        if self.variant {
            end_variant(self.e);
        }
        Ok(())
    }
}

impl ser::SerializeMap for Map<'_> {
    type Ok = ();
    type Error = Error;

    fn serialize_key<T: Serialize + ?Sized>(&mut self, key: &T) -> Result<(), Error> {
        self.entries.key_with(|e| serialize_into(key, e))
    }

    fn serialize_value<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), Error> {
        self.entries.value_with(|e| serialize_into(value, e))
    }

    fn end(self) -> Result<(), Error> {
        self.finish()
    }
}

impl ser::SerializeStruct for Map<'_> {
    type Ok = ();
    type Error = Error;

    fn serialize_field<T: Serialize + ?Sized>(
        &mut self,
        key: &'static str,
        value: &T,
    ) -> Result<(), Error> {
        self.field(key, value)
    }

    fn end(self) -> Result<(), Error> {
        self.finish()
    }
}

impl ser::SerializeStructVariant for Map<'_> {
    type Ok = ();
    type Error = Error;

    fn serialize_field<T: Serialize + ?Sized>(
        &mut self,
        key: &'static str,
        value: &T,
    ) -> Result<(), Error> {
        self.field(key, value)
    }

    fn end(self) -> Result<(), Error> {
        self.finish()
    }
}
