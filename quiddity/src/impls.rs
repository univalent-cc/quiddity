//! `Quiddity` for standard library types (spec 5.2). Every impl covers
//! every identity, encoding elements and fields under the same identity, so
//! that `identity = I` reaches through containers.

use core::marker::PhantomData;
use core::num::NonZero;

use crate::{Digest, Digest64, Digest128, Encoder, IdentityMarker, Quiddity};

macro_rules! integers {
    ($($t:ty)*) => {$(
        impl<I: IdentityMarker> Quiddity<I> for $t {
            fn encode(&self, e: &mut Encoder) {
                e.int(*self);
            }
        }

        impl<I: IdentityMarker> Quiddity<I> for NonZero<$t> {
            fn encode(&self, e: &mut Encoder) {
                e.int(self.get());
            }
        }
    )*};
}

integers!(i8 i16 i32 i64 i128 isize u8 u16 u32 u64 u128 usize);

impl<I: IdentityMarker> Quiddity<I> for f64 {
    fn encode(&self, e: &mut Encoder) {
        e.float(*self);
    }
}

impl<I: IdentityMarker> Quiddity<I> for f32 {
    fn encode(&self, e: &mut Encoder) {
        e.float(f64::from(*self));
    }
}

impl<I: IdentityMarker> Quiddity<I> for bool {
    fn encode(&self, e: &mut Encoder) {
        e.bool(*self);
    }
}

impl<I: IdentityMarker> Quiddity<I> for char {
    fn encode(&self, e: &mut Encoder) {
        e.str(self.encode_utf8(&mut [0; 4]));
    }
}

impl<I: IdentityMarker> Quiddity<I> for str {
    fn encode(&self, e: &mut Encoder) {
        e.str(self);
    }
}

impl<I: IdentityMarker> Quiddity<I> for () {
    fn encode(&self, e: &mut Encoder) {
        e.null();
    }
}

/// Null as a value. As a field of a derived type, absent: the derive skips
/// it under every identity.
impl<I: IdentityMarker, T: ?Sized> Quiddity<I> for PhantomData<T> {
    #[doc(hidden)]
    const ABSENT_AS_FIELD: bool = true;

    fn encode(&self, e: &mut Encoder) {
        e.null();
    }
}

impl<I: IdentityMarker, T: Quiddity<I>> Quiddity<I> for Option<T> {
    fn encode(&self, e: &mut Encoder) {
        match self {
            None => e.null(),
            Some(x) => x.encode(e),
        }
    }
}

/// Encodes `items` as a list, under identity `I`.
fn list<'a, I, T>(e: &mut Encoder, items: impl IntoIterator<Item = &'a T>)
where
    I: IdentityMarker,
    T: Quiddity<I> + 'a,
{
    let mut l = e.list();
    for item in items {
        l.item_as::<I, T>(item);
    }
}

impl<I: IdentityMarker, T: Quiddity<I>> Quiddity<I> for [T] {
    fn encode(&self, e: &mut Encoder) {
        list::<I, T>(e, self);
    }
}

impl<I: IdentityMarker, T: Quiddity<I>, const N: usize> Quiddity<I> for [T; N] {
    fn encode(&self, e: &mut Encoder) {
        list::<I, T>(e, self);
    }
}

macro_rules! tuples {
    ($(($($t:ident $i:tt),+))*) => {$(
        impl<I: IdentityMarker, $($t: Quiddity<I>),+> Quiddity<I> for ($($t,)+) {
            fn encode(&self, e: &mut Encoder) {
                let mut l = e.list();
                $(l.item_as::<I, $t>(&self.$i);)+
            }
        }
    )*};
}

tuples! {
    (A 0)
    (A 0, B 1)
    (A 0, B 1, C 2)
    (A 0, B 1, C 2, D 3)
    (A 0, B 1, C 2, D 3, E 4)
    (A 0, B 1, C 2, D 3, E 4, F 5)
    (A 0, B 1, C 2, D 3, E 4, F 5, G 6)
    (A 0, B 1, C 2, D 3, E 4, F 5, G 6, H 7)
    (A 0, B 1, C 2, D 3, E 4, F 5, G 6, H 7, J 8)
    (A 0, B 1, C 2, D 3, E 4, F 5, G 6, H 7, J 8, K 9)
    (A 0, B 1, C 2, D 3, E 4, F 5, G 6, H 7, J 8, K 9, L 10)
    (A 0, B 1, C 2, D 3, E 4, F 5, G 6, H 7, J 8, K 9, L 10, M 11)
}

/// Pointer-like types encode as the value they point to, in its domain.
macro_rules! transparent {
    ($($(#[$cfg:meta])* $ty:ty;)*) => {$(
        $(#[$cfg])*
        impl<I: IdentityMarker, T: Quiddity<I> + ?Sized> Quiddity<I> for $ty {
            const DOMAIN: Option<&'static str> = T::DOMAIN;

            fn encode(&self, e: &mut Encoder) {
                T::encode(self, e);
            }
        }
    )*};
}

transparent! {
    &T;
    &mut T;
    #[cfg(feature = "alloc")]
    alloc::boxed::Box<T>;
    #[cfg(feature = "alloc")]
    alloc::rc::Rc<T>;
    #[cfg(all(feature = "alloc", target_has_atomic = "ptr"))]
    alloc::sync::Arc<T>;
}

#[cfg(feature = "alloc")]
impl<I, T> Quiddity<I> for alloc::borrow::Cow<'_, T>
where
    I: IdentityMarker,
    T: Quiddity<I> + alloc::borrow::ToOwned + ?Sized,
{
    const DOMAIN: Option<&'static str> = T::DOMAIN;

    fn encode(&self, e: &mut Encoder) {
        T::encode(self, e);
    }
}

#[cfg(feature = "alloc")]
pub(crate) mod alloc_impls {
    use alloc::collections::{BTreeMap, BTreeSet, VecDeque};
    use alloc::string::String;
    use alloc::vec::Vec;

    use super::list;
    use crate::{Encoder, IdentityMarker, Quiddity};

    impl<I: IdentityMarker> Quiddity<I> for String {
        fn encode(&self, e: &mut Encoder) {
            e.str(self);
        }
    }

    impl<I: IdentityMarker, T: Quiddity<I>> Quiddity<I> for Vec<T> {
        fn encode(&self, e: &mut Encoder) {
            list::<I, T>(e, self);
        }
    }

    impl<I: IdentityMarker, T: Quiddity<I>> Quiddity<I> for VecDeque<T> {
        fn encode(&self, e: &mut Encoder) {
            list::<I, T>(e, self);
        }
    }

    impl<I: IdentityMarker, T: Quiddity<I>> Quiddity<I> for BTreeSet<T> {
        fn encode(&self, e: &mut Encoder) {
            set::<I, T>(e, self);
        }
    }

    impl<I: IdentityMarker, K: Quiddity<I>, V: Quiddity<I>> Quiddity<I> for BTreeMap<K, V> {
        fn encode(&self, e: &mut Encoder) {
            map::<I, K, V>(e, self);
        }
    }

    /// Encodes `members` as a set, under identity `I`.
    pub(crate) fn set<'a, I, T>(e: &mut Encoder, members: impl IntoIterator<Item = &'a T>)
    where
        I: IdentityMarker,
        T: Quiddity<I> + 'a,
    {
        let mut s = e.set();
        for member in members {
            s.member_as::<I, T>(member);
        }
    }

    /// Encodes `entries` as a map, under identity `I`.
    pub(crate) fn map<'a, I, K, V>(
        e: &mut Encoder,
        entries: impl IntoIterator<Item = (&'a K, &'a V)>,
    ) where
        I: IdentityMarker,
        K: Quiddity<I> + 'a,
        V: Quiddity<I> + 'a,
    {
        let mut m = e.map();
        for (k, v) in entries {
            m.entry_as::<I, K, V>(k, v);
        }
    }
}

#[cfg(feature = "std")]
mod std_impls {
    use std::collections::{HashMap, HashSet};

    use super::alloc_impls::{map, set};
    use crate::{Encoder, IdentityMarker, Quiddity};

    impl<I: IdentityMarker, T: Quiddity<I>, S> Quiddity<I> for HashSet<T, S> {
        fn encode(&self, e: &mut Encoder) {
            set::<I, T>(e, self);
        }
    }

    impl<I: IdentityMarker, K: Quiddity<I>, V: Quiddity<I>, S> Quiddity<I> for HashMap<K, V, S> {
        fn encode(&self, e: &mut Encoder) {
            map::<I, K, V>(e, self);
        }
    }
}

/// A digest used as a value is the string of its text form (spec 5.2).
macro_rules! digests {
    ($($t:ty)*) => {$(
        impl<I: IdentityMarker> Quiddity<I> for $t {
            fn encode(&self, e: &mut Encoder) {
                e.display(self);
            }
        }
    )*};
}

digests!(Digest Digest128 Digest64);
