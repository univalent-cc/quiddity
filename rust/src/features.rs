//! `Quiddity` for other crates' types, each behind a feature (spec 6.6).

#[cfg(feature = "serde_json")]
mod serde_json_impls {
    use alloc::string::{String, ToString};

    use serde_json::{Map, Number, Value};

    use crate::impls::alloc_impls::map;
    use crate::{Encoder, IdentityMarker, Quiddity};

    /// JSON semantics: objects are unordered, numbers are compared by value.
    impl<I: IdentityMarker> Quiddity<I> for Value {
        describe!(|cx| Kd::Any);

        fn encode(&self, e: &mut Encoder) {
            match self {
                Value::Null => e.null(),
                Value::Bool(b) => e.bool(*b),
                Value::Number(n) => Quiddity::<I>::encode(n, e),
                Value::String(s) => e.str(s),
                Value::Array(items) => Quiddity::<I>::encode(items, e),
                Value::Object(m) => Quiddity::<I>::encode(m, e),
            }
        }
    }

    impl<I: IdentityMarker> Quiddity<I> for Map<String, Value> {
        describe!(|cx| X::map(Kd::String, Kd::Any));
        flatten!(|this, m| for (k, v) in this {
            m.entry_as::<I, String, Value>(k, v);
        });

        fn encode(&self, e: &mut Encoder) {
            map::<I, String, Value>(e, self);
        }
    }

    /// The value the number would have without serde_json's
    /// `arbitrary_precision` feature (spec 6.6): an integer within `i64` or
    /// `u64`, or else the nearest `f64`.
    impl<I: IdentityMarker> Quiddity<I> for Number {
        describe!(|cx| Kd::Number);

        fn encode(&self, e: &mut Encoder) {
            if let Some(i) = self.as_i64() {
                e.int(i);
            } else if let Some(u) = self.as_u64() {
                e.int(u);
            } else if let Some(f) = self.as_f64() {
                e.float(f);
            } else {
                // Only with `arbitrary_precision`, for text beyond the range
                // of `f64`: rounding gives an infinity.
                e.float(self.to_string().parse().unwrap_or(f64::NAN));
            }
        }
    }
}

#[cfg(feature = "indexmap")]
mod indexmap_impls {
    use indexmap::{IndexMap, IndexSet};

    use crate::impls::alloc_impls::{map, set};
    use crate::{Encoder, IdentityMarker, Quiddity};

    impl<I: IdentityMarker, K: Quiddity<I>, V: Quiddity<I>, S> Quiddity<I> for IndexMap<K, V, S> {
        describe!(|cx| X::map(cx.describe::<K, I>()?, cx.describe::<V, I>()?));
        flatten!(|this, m| for (k, v) in this {
            m.entry_as::<I, K, V>(k, v);
        });

        fn encode(&self, e: &mut Encoder) {
            map::<I, K, V>(e, self);
        }
    }

    impl<I: IdentityMarker, T: Quiddity<I>, S> Quiddity<I> for IndexSet<T, S> {
        describe!(|cx| Kd::set(cx.describe::<T, I>()?));

        fn encode(&self, e: &mut Encoder) {
            set::<I, T>(e, self);
        }
    }
}

/// The hyphenated lowercase string, as serde writes it in JSON.
#[cfg(feature = "uuid")]
impl<I: crate::IdentityMarker> crate::Quiddity<I> for uuid::Uuid {
    describe!(|cx| Kd::Uuid);

    fn encode(&self, e: &mut crate::Encoder) {
        e.str(
            self.hyphenated()
                .encode_lower(&mut uuid::Uuid::encode_buffer()),
        );
    }
}

#[cfg(feature = "bytes")]
mod bytes_impls {
    use bytes::{Bytes, BytesMut};

    use crate::{Encoder, IdentityMarker, Quiddity};

    impl<I: IdentityMarker> Quiddity<I> for Bytes {
        describe!(|cx| Kd::Bytes(Default::default()));

        fn encode(&self, e: &mut Encoder) {
            e.bytes(self);
        }
    }

    impl<I: IdentityMarker> Quiddity<I> for BytesMut {
        describe!(|cx| Kd::Bytes(Default::default()));

        fn encode(&self, e: &mut Encoder) {
            e.bytes(self);
        }
    }
}

#[cfg(feature = "url")]
impl<I: crate::IdentityMarker> crate::Quiddity<I> for url::Url {
    describe!(|cx| Kd::String);

    fn encode(&self, e: &mut crate::Encoder) {
        e.str(self.as_str());
    }
}
