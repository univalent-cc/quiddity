//! `flatten` (D21): a field whose members are written into the map of the
//! struct that contains it, as serde's `flatten` writes them, so that the
//! digest is that of the flattened object.

use crate::encoder::Entries;
use crate::{Encoder, IdentityMarker, Quiddity};

/// The map of a struct with a flattened field: its own fields and the
/// flattened members, in canonical order (spec 2.5) once finished.
#[must_use = "a flattened map is written by `finish`"]
pub struct Flattened<'e> {
    e: &'e mut Encoder,
    entries: Entries,
    /// The type being digested, for the message about a repeated key.
    type_name: &'static str,
}

/// The map of a struct of type `type_name` with a flattened field.
pub fn flattened<'e>(e: &'e mut Encoder, type_name: &'static str) -> Flattened<'e> {
    Flattened {
        e,
        entries: Entries::default(),
        type_name,
    }
}

impl Flattened<'_> {
    /// Adds the member `name`, whose value is the one value `f` writes.
    pub fn field_with(&mut self, name: &str, f: impl FnOnce(&mut Encoder)) {
        self.entries.key_with(|e| e.str(name));
        self.entries.value_with(f);
    }

    /// Adds an entry with key and value under identity `I`.
    pub fn entry_as<I, K, V>(&mut self, key: &K, value: &V)
    where
        I: IdentityMarker,
        K: Quiddity<I> + ?Sized,
        V: Quiddity<I> + ?Sized,
    {
        self.entries
            .key_with(|e| <K as Quiddity<I>>::encode(key, e));
        self.entries
            .value_with(|e| <V as Quiddity<I>>::encode(value, e));
    }

    /// Panics: `what` has no members to flatten, as serde reports when
    /// serializing it.
    pub fn cannot_flatten(&self, what: &str) -> ! {
        panic!(
            "quiddity: cannot flatten {what} while digesting `{}`: only structs, maps and \
             tagged enums have members to flatten",
            self.type_name
        );
    }

    /// Writes the map.
    ///
    /// # Panics
    ///
    /// If a key is written twice: a flattened member with the name of
    /// another field.
    pub fn finish(self) {
        let type_name = self.type_name;
        self.entries.write_sorted_unique(self.e, |key| {
            let key = key_text(key);
            panic!(
                "quiddity: the key {key} is written twice while digesting `{type_name}`: a \
                 flattened field (`#[quiddity(flatten)]` or `#[serde(flatten)]`) has a member \
                 with the same name as another field. Its JSON form would have a repeated key, \
                 which has no single meaning (spec 2.7). Rename one of them."
            )
        });
    }
}

/// A key's encoding as text for a message: a string key quoted, any other
/// key described.
fn key_text(encoding: &[u8]) -> alloc::string::String {
    use alloc::format;
    if let [content @ .., l0, l1, l2, l3, l4, l5, l6, l7, 0x20] = encoding {
        let len = u64::from_be_bytes([*l0, *l1, *l2, *l3, *l4, *l5, *l6, *l7]);
        if len == content.len() as u64 {
            return format!("{:?}", alloc::string::String::from_utf8_lossy(content));
        }
    }
    "(not a string)".into()
}
