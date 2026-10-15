//! The source values identities are applied to (spec 5.4): a tree that a
//! `Serialize` value writes through [`to_source`], in the forms serde_json
//! gives it, so that a value and its `serde_json::Value` form are read the
//! same way. Where `serde_json::Value` cannot hold a value, the tree keeps
//! it: integers of any size, and non-finite floats.

use alloc::borrow::ToOwned;
use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use ::serde::ser::{self, Serialize};

use super::value::Decimal;
use crate::serde::Error;

/// A source value.
#[derive(Debug)]
pub(crate) enum Src {
    Null,
    Bool(bool),
    /// An integer, as big-endian two's-complement bytes.
    Int(Vec<u8>),
    Float(f64),
    Str(String),
    Array(Vec<Src>),
    /// An object's members. Member order does not matter (spec 2.7).
    Object(BTreeMap<String, Src>),
}

impl Src {
    pub(crate) fn int(n: i128) -> Src {
        Src::Int(n.to_be_bytes().to_vec())
    }

    pub(crate) fn as_str(&self) -> Option<&str> {
        match self {
            Src::Str(s) => Some(s),
            _ => None,
        }
    }

    pub(crate) fn as_array(&self) -> Option<&[Src]> {
        match self {
            Src::Array(items) => Some(items),
            _ => None,
        }
    }

    pub(crate) fn as_object(&self) -> Option<&BTreeMap<String, Src>> {
        match self {
            Src::Object(o) => Some(o),
            _ => None,
        }
    }

    /// The integer, if this is one that fits in an `i128`.
    pub(crate) fn as_i128(&self) -> Option<i128> {
        match self {
            Src::Int(bytes) => {
                let b = crate::number::minimal(bytes);
                if b.len() > 16 {
                    return None;
                }
                let fill = if b[0] & 0x80 != 0 { 0xFF } else { 0 };
                let mut buf = [fill; 16];
                buf[16 - b.len()..].copy_from_slice(b);
                Some(i128::from_be_bytes(buf))
            }
            _ => None,
        }
    }

    /// The integer, if this is one from 0 to `u64::MAX`.
    pub(crate) fn as_u64(&self) -> Option<u64> {
        self.as_i128().and_then(|i| u64::try_from(i).ok())
    }
}

/// The source tree of `value`.
pub(crate) fn to_source<T: Serialize + ?Sized>(value: &T) -> Result<Src, Error> {
    value.serialize(Serializer)
}

/// A JSON number's text as spec 2.7 maps it: an integer if it has no
/// fraction or exponent and fits in 64 bits, else the nearest binary64.
pub(crate) fn json_number(text: &str) -> Option<Src> {
    let d = Decimal::parse(text)?;
    if d.plain_integer {
        if let Ok(i) = d.body.parse::<i64>() {
            return Some(Src::int(i.into()));
        }
        if let Ok(u) = d.body.parse::<u64>() {
            return Some(Src::int(u.into()));
        }
    }
    d.body.parse().ok().map(Src::Float)
}

/// The name under which serde_json's `arbitrary_precision` feature writes a
/// number, as a struct with one field of this name holding its text.
const NUMBER_TOKEN: &str = "$serde_json::private::Number";

/// The name under which serde_json's `raw_value` feature writes raw JSON.
const RAW_TOKEN: &str = "$serde_json::private::RawValue";

fn error(message: &str) -> Error {
    <Error as ser::Error>::custom(message)
}

struct Serializer;

impl ser::Serializer for Serializer {
    type Ok = Src;
    type Error = Error;
    type SerializeSeq = Seq;
    type SerializeTuple = Seq;
    type SerializeTupleStruct = Seq;
    type SerializeTupleVariant = Seq;
    type SerializeMap = Map;
    type SerializeStruct = Map;
    type SerializeStructVariant = Map;

    fn serialize_bool(self, v: bool) -> Result<Src, Error> {
        Ok(Src::Bool(v))
    }

    fn serialize_i8(self, v: i8) -> Result<Src, Error> {
        Ok(Src::int(v.into()))
    }

    fn serialize_i16(self, v: i16) -> Result<Src, Error> {
        Ok(Src::int(v.into()))
    }

    fn serialize_i32(self, v: i32) -> Result<Src, Error> {
        Ok(Src::int(v.into()))
    }

    fn serialize_i64(self, v: i64) -> Result<Src, Error> {
        Ok(Src::int(v.into()))
    }

    fn serialize_i128(self, v: i128) -> Result<Src, Error> {
        Ok(Src::int(v))
    }

    fn serialize_u8(self, v: u8) -> Result<Src, Error> {
        Ok(Src::int(v.into()))
    }

    fn serialize_u16(self, v: u16) -> Result<Src, Error> {
        Ok(Src::int(v.into()))
    }

    fn serialize_u32(self, v: u32) -> Result<Src, Error> {
        Ok(Src::int(v.into()))
    }

    fn serialize_u64(self, v: u64) -> Result<Src, Error> {
        Ok(Src::int(v.into()))
    }

    fn serialize_u128(self, v: u128) -> Result<Src, Error> {
        // With a leading zero byte, so that it is not negative.
        let mut bytes = Vec::with_capacity(17);
        bytes.push(0);
        bytes.extend_from_slice(&v.to_be_bytes());
        Ok(Src::Int(bytes))
    }

    /// Widened exactly, as serde_json does: `0.1f32` is not `0.1f64`.
    fn serialize_f32(self, v: f32) -> Result<Src, Error> {
        Ok(Src::Float(v.into()))
    }

    fn serialize_f64(self, v: f64) -> Result<Src, Error> {
        Ok(Src::Float(v))
    }

    fn serialize_char(self, v: char) -> Result<Src, Error> {
        Ok(Src::Str(v.to_string()))
    }

    fn serialize_str(self, v: &str) -> Result<Src, Error> {
        Ok(Src::Str(v.to_owned()))
    }

    /// An array of integers, as serde_json writes bytes.
    fn serialize_bytes(self, v: &[u8]) -> Result<Src, Error> {
        Ok(Src::Array(
            v.iter().map(|b| Src::int((*b).into())).collect(),
        ))
    }

    fn serialize_none(self) -> Result<Src, Error> {
        Ok(Src::Null)
    }

    fn serialize_some<T: Serialize + ?Sized>(self, value: &T) -> Result<Src, Error> {
        value.serialize(self)
    }

    fn serialize_unit(self) -> Result<Src, Error> {
        Ok(Src::Null)
    }

    fn serialize_unit_struct(self, _name: &'static str) -> Result<Src, Error> {
        Ok(Src::Null)
    }

    fn serialize_unit_variant(
        self,
        _name: &'static str,
        _index: u32,
        variant: &'static str,
    ) -> Result<Src, Error> {
        Ok(Src::Str(variant.to_owned()))
    }

    fn serialize_newtype_struct<T: Serialize + ?Sized>(
        self,
        _name: &'static str,
        value: &T,
    ) -> Result<Src, Error> {
        value.serialize(self)
    }

    fn serialize_newtype_variant<T: Serialize + ?Sized>(
        self,
        _name: &'static str,
        _index: u32,
        variant: &'static str,
        value: &T,
    ) -> Result<Src, Error> {
        Ok(Src::Object(BTreeMap::from([(
            variant.to_owned(),
            value.serialize(self)?,
        )])))
    }

    fn serialize_seq(self, len: Option<usize>) -> Result<Seq, Error> {
        Ok(Seq {
            variant: None,
            items: Vec::with_capacity(len.unwrap_or(0)),
        })
    }

    fn serialize_tuple(self, len: usize) -> Result<Seq, Error> {
        self.serialize_seq(Some(len))
    }

    fn serialize_tuple_struct(self, _name: &'static str, len: usize) -> Result<Seq, Error> {
        self.serialize_seq(Some(len))
    }

    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        _index: u32,
        variant: &'static str,
        len: usize,
    ) -> Result<Seq, Error> {
        Ok(Seq {
            variant: Some(variant),
            items: Vec::with_capacity(len),
        })
    }

    fn serialize_map(self, _len: Option<usize>) -> Result<Map, Error> {
        Ok(Map::new(None))
    }

    fn serialize_struct(self, name: &'static str, _len: usize) -> Result<Map, Error> {
        if name == RAW_TOKEN {
            return Err(error("raw JSON values are not supported"));
        }
        Ok(Map::new(None))
    }

    fn serialize_struct_variant(
        self,
        _name: &'static str,
        _index: u32,
        variant: &'static str,
        _len: usize,
    ) -> Result<Map, Error> {
        Ok(Map::new(Some(variant)))
    }
}

/// A sequence being written; a tuple variant's, inside `{variant: […]}`.
pub(crate) struct Seq {
    variant: Option<&'static str>,
    items: Vec<Src>,
}

impl Seq {
    fn push<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), Error> {
        self.items.push(value.serialize(Serializer)?);
        Ok(())
    }

    fn finish(self) -> Src {
        let array = Src::Array(self.items);
        match self.variant {
            Some(v) => Src::Object(BTreeMap::from([(v.to_owned(), array)])),
            None => array,
        }
    }
}

impl ser::SerializeSeq for Seq {
    type Ok = Src;
    type Error = Error;

    fn serialize_element<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), Error> {
        self.push(value)
    }

    fn end(self) -> Result<Src, Error> {
        Ok(self.finish())
    }
}

impl ser::SerializeTuple for Seq {
    type Ok = Src;
    type Error = Error;

    fn serialize_element<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), Error> {
        self.push(value)
    }

    fn end(self) -> Result<Src, Error> {
        Ok(self.finish())
    }
}

impl ser::SerializeTupleStruct for Seq {
    type Ok = Src;
    type Error = Error;

    fn serialize_field<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), Error> {
        self.push(value)
    }

    fn end(self) -> Result<Src, Error> {
        Ok(self.finish())
    }
}

impl ser::SerializeTupleVariant for Seq {
    type Ok = Src;
    type Error = Error;

    fn serialize_field<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), Error> {
        self.push(value)
    }

    fn end(self) -> Result<Src, Error> {
        Ok(self.finish())
    }
}

/// An object being written; a struct variant's, inside `{variant: {…}}`.
pub(crate) struct Map {
    variant: Option<&'static str>,
    members: BTreeMap<String, Src>,
    key: Option<String>,
    /// serde_json's `arbitrary_precision` number, once its text is written.
    number: Option<Src>,
}

impl Map {
    fn new(variant: Option<&'static str>) -> Map {
        Map {
            variant,
            members: BTreeMap::new(),
            key: None,
            number: None,
        }
    }

    fn insert(&mut self, key: String, value: Src) -> Result<(), Error> {
        if self.members.contains_key(&key) {
            return Err(<Error as ser::Error>::custom(alloc::format!(
                "the member {key:?} is written twice"
            )));
        }
        self.members.insert(key, value);
        Ok(())
    }

    fn finish(self) -> Src {
        if let Some(n) = self.number {
            return n;
        }
        let object = Src::Object(self.members);
        match self.variant {
            Some(v) => Src::Object(BTreeMap::from([(v.to_owned(), object)])),
            None => object,
        }
    }
}

impl ser::SerializeMap for Map {
    type Ok = Src;
    type Error = Error;

    fn serialize_key<T: Serialize + ?Sized>(&mut self, key: &T) -> Result<(), Error> {
        self.key = Some(key.serialize(KeySerializer)?);
        Ok(())
    }

    fn serialize_value<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), Error> {
        let key = self
            .key
            .take()
            .ok_or_else(|| error("a map value was written before its key"))?;
        let value = value.serialize(Serializer)?;
        self.insert(key, value)
    }

    fn end(self) -> Result<Src, Error> {
        Ok(self.finish())
    }
}

impl ser::SerializeStruct for Map {
    type Ok = Src;
    type Error = Error;

    fn serialize_field<T: Serialize + ?Sized>(
        &mut self,
        key: &'static str,
        value: &T,
    ) -> Result<(), Error> {
        let value = value.serialize(Serializer)?;
        if key == NUMBER_TOKEN {
            // The number's text, as spec 2.7 maps JSON numbers.
            let n = value
                .as_str()
                .and_then(json_number)
                .ok_or_else(|| error("an arbitrary-precision number is not valid"))?;
            self.number = Some(n);
            return Ok(());
        }
        self.insert(key.to_owned(), value)
    }

    fn end(self) -> Result<Src, Error> {
        Ok(self.finish())
    }
}

impl ser::SerializeStructVariant for Map {
    type Ok = Src;
    type Error = Error;

    fn serialize_field<T: Serialize + ?Sized>(
        &mut self,
        key: &'static str,
        value: &T,
    ) -> Result<(), Error> {
        let value = value.serialize(Serializer)?;
        self.insert(key.to_owned(), value)
    }

    fn end(self) -> Result<Src, Error> {
        Ok(self.finish())
    }
}

/// A map key, as serde_json writes one: a string, or the text of a
/// boolean or an integer. Float keys, which serde_json writes as text, are
/// not accepted.
struct KeySerializer;

fn key_error() -> Error {
    error("a map key must be a string, a boolean, an integer or a unit variant")
}

impl ser::Serializer for KeySerializer {
    type Ok = String;
    type Error = Error;
    type SerializeSeq = ser::Impossible<String, Error>;
    type SerializeTuple = ser::Impossible<String, Error>;
    type SerializeTupleStruct = ser::Impossible<String, Error>;
    type SerializeTupleVariant = ser::Impossible<String, Error>;
    type SerializeMap = ser::Impossible<String, Error>;
    type SerializeStruct = ser::Impossible<String, Error>;
    type SerializeStructVariant = ser::Impossible<String, Error>;

    fn serialize_bool(self, v: bool) -> Result<String, Error> {
        Ok(if v { "true" } else { "false" }.to_owned())
    }

    fn serialize_i8(self, v: i8) -> Result<String, Error> {
        Ok(v.to_string())
    }

    fn serialize_i16(self, v: i16) -> Result<String, Error> {
        Ok(v.to_string())
    }

    fn serialize_i32(self, v: i32) -> Result<String, Error> {
        Ok(v.to_string())
    }

    fn serialize_i64(self, v: i64) -> Result<String, Error> {
        Ok(v.to_string())
    }

    fn serialize_i128(self, v: i128) -> Result<String, Error> {
        Ok(v.to_string())
    }

    fn serialize_u8(self, v: u8) -> Result<String, Error> {
        Ok(v.to_string())
    }

    fn serialize_u16(self, v: u16) -> Result<String, Error> {
        Ok(v.to_string())
    }

    fn serialize_u32(self, v: u32) -> Result<String, Error> {
        Ok(v.to_string())
    }

    fn serialize_u64(self, v: u64) -> Result<String, Error> {
        Ok(v.to_string())
    }

    fn serialize_u128(self, v: u128) -> Result<String, Error> {
        Ok(v.to_string())
    }

    fn serialize_f32(self, _v: f32) -> Result<String, Error> {
        Err(key_error())
    }

    fn serialize_f64(self, _v: f64) -> Result<String, Error> {
        Err(key_error())
    }

    fn serialize_char(self, v: char) -> Result<String, Error> {
        Ok(v.to_string())
    }

    fn serialize_str(self, v: &str) -> Result<String, Error> {
        Ok(v.to_owned())
    }

    fn serialize_bytes(self, _v: &[u8]) -> Result<String, Error> {
        Err(key_error())
    }

    fn serialize_none(self) -> Result<String, Error> {
        Err(key_error())
    }

    fn serialize_some<T: Serialize + ?Sized>(self, _value: &T) -> Result<String, Error> {
        Err(key_error())
    }

    fn serialize_unit(self) -> Result<String, Error> {
        Err(key_error())
    }

    fn serialize_unit_struct(self, _name: &'static str) -> Result<String, Error> {
        Err(key_error())
    }

    fn serialize_unit_variant(
        self,
        _name: &'static str,
        _index: u32,
        variant: &'static str,
    ) -> Result<String, Error> {
        Ok(variant.to_owned())
    }

    fn serialize_newtype_struct<T: Serialize + ?Sized>(
        self,
        _name: &'static str,
        value: &T,
    ) -> Result<String, Error> {
        value.serialize(self)
    }

    fn serialize_newtype_variant<T: Serialize + ?Sized>(
        self,
        _name: &'static str,
        _index: u32,
        _variant: &'static str,
        _value: &T,
    ) -> Result<String, Error> {
        Err(key_error())
    }

    fn serialize_seq(self, _len: Option<usize>) -> Result<Self::SerializeSeq, Error> {
        Err(key_error())
    }

    fn serialize_tuple(self, _len: usize) -> Result<Self::SerializeTuple, Error> {
        Err(key_error())
    }

    fn serialize_tuple_struct(
        self,
        _name: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleStruct, Error> {
        Err(key_error())
    }

    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        _index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleVariant, Error> {
        Err(key_error())
    }

    fn serialize_map(self, _len: Option<usize>) -> Result<Self::SerializeMap, Error> {
        Err(key_error())
    }

    fn serialize_struct(
        self,
        _name: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStruct, Error> {
        Err(key_error())
    }

    fn serialize_struct_variant(
        self,
        _name: &'static str,
        _index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStructVariant, Error> {
        Err(key_error())
    }
}
