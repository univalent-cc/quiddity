//! `Quiddity` for standard library types (spec 6.2). Every impl covers
//! every identity, encoding elements and fields under the same identity, so
//! that `identity = I` reaches through containers.

use core::marker::PhantomData;
use core::num::NonZero;

use crate::{Digest, Digest64, Digest128, Encoder, IdentityMarker, Quiddity};

macro_rules! integers {
    ($($t:ty)*) => {$(
        impl<I: IdentityMarker> Quiddity<I> for $t {
            describe!(|cx| Kd::Integer);

            fn encode(&self, e: &mut Encoder) {
                e.int(*self);
            }
        }

        impl<I: IdentityMarker> Quiddity<I> for NonZero<$t> {
            describe!(|cx| Kd::Integer);

            fn encode(&self, e: &mut Encoder) {
                e.int(self.get());
            }
        }
    )*};
}

integers!(i8 i16 i32 i64 i128 isize u8 u16 u32 u64 u128 usize);

impl<I: IdentityMarker> Quiddity<I> for f64 {
    describe!(|cx| Kd::Number);

    fn encode(&self, e: &mut Encoder) {
        e.float(*self);
    }
}

impl<I: IdentityMarker> Quiddity<I> for f32 {
    describe!(|cx| Kd::Number);

    fn encode(&self, e: &mut Encoder) {
        e.float(f64::from(*self));
    }
}

impl<I: IdentityMarker> Quiddity<I> for bool {
    describe!(|cx| Kd::Bool);

    fn encode(&self, e: &mut Encoder) {
        e.bool(*self);
    }
}

impl<I: IdentityMarker> Quiddity<I> for char {
    describe!(|cx| Kd::String);

    fn encode(&self, e: &mut Encoder) {
        e.str(self.encode_utf8(&mut [0; 4]));
    }
}

impl<I: IdentityMarker> Quiddity<I> for str {
    describe!(|cx| Kd::String);

    fn encode(&self, e: &mut Encoder) {
        e.str(self);
    }
}

impl<I: IdentityMarker> Quiddity<I> for () {
    describe!(|cx| Kd::Null);

    fn encode(&self, e: &mut Encoder) {
        e.null();
    }
}

/// Null as a value. As a field of a derived type, absent: the derive skips
/// it under every identity.
impl<I: IdentityMarker, T: ?Sized> Quiddity<I> for PhantomData<T> {
    describe!(|cx| Kd::Null);

    #[doc(hidden)]
    const ABSENT_AS_FIELD: bool = true;

    fn encode(&self, e: &mut Encoder) {
        e.null();
    }
}

impl<I: IdentityMarker, T: Quiddity<I>> Quiddity<I> for Option<T> {
    describe!(|cx| Kd::optional(cx.describe::<T, I>()?));

    // `None` has no members.
    #[cfg(feature = "alloc")]
    const __FLATTENABLE: bool = T::__FLATTENABLE;

    #[cfg(feature = "alloc")]
    fn __encode_entries(&self, m: &mut crate::flatten::Flattened<'_>) {
        if let Some(x) = self {
            x.__encode_entries(m);
        }
    }

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
    describe!(|cx| Kd::list(cx.describe::<T, I>()?));

    fn encode(&self, e: &mut Encoder) {
        list::<I, T>(e, self);
    }
}

impl<I: IdentityMarker, T: Quiddity<I>, const N: usize> Quiddity<I> for [T; N] {
    describe!(|cx| Kd::list(cx.describe::<T, I>()?));

    fn encode(&self, e: &mut Encoder) {
        list::<I, T>(e, self);
    }
}

macro_rules! tuples {
    ($(($($t:ident $i:tt),+))*) => {$(
        impl<I: IdentityMarker, $($t: Quiddity<I>),+> Quiddity<I> for ($($t,)+) {
            describe!(|cx| Kd::Tuple(alloc::vec![$(cx.describe::<$t, I>()?),+]));

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
            describe!(|cx| cx.describe::<T, I>()?);

            const DOMAIN: Option<&'static str> = T::DOMAIN;

            #[cfg(feature = "alloc")]
            const __FLATTENABLE: bool = T::__FLATTENABLE;

            fn encode(&self, e: &mut Encoder) {
                T::encode(self, e);
            }

            #[cfg(feature = "alloc")]
            fn __encode_entries(&self, m: &mut crate::flatten::Flattened<'_>) {
                T::__encode_entries(self, m);
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
    describe!(|cx| cx.describe::<T, I>()?);

    const DOMAIN: Option<&'static str> = T::DOMAIN;

    fn encode(&self, e: &mut Encoder) {
        T::encode(self, e);
    }
}

impl<I, P> Quiddity<I> for core::pin::Pin<P>
where
    I: IdentityMarker,
    P: core::ops::Deref,
    P::Target: Quiddity<I>,
{
    describe!(|cx| cx.describe::<P::Target, I>()?);

    const DOMAIN: Option<&'static str> = <P::Target as Quiddity<I>>::DOMAIN;

    fn encode(&self, e: &mut Encoder) {
        <P::Target as Quiddity<I>>::encode(self, e);
    }
}

/// Wrappers encode as the value they wrap.
macro_rules! wrappers {
    ($($ty:ident)*) => {$(
        impl<I: IdentityMarker, T: Quiddity<I>> Quiddity<I> for $ty<T> {
            describe!(|cx| cx.describe::<T, I>()?);

            fn encode(&self, e: &mut Encoder) {
                self.0.encode(e);
            }
        }
    )*};
}

use core::cmp::Reverse;
use core::num::{Saturating, Wrapping};
wrappers!(Wrapping Saturating Reverse);

impl<I: IdentityMarker, T: Quiddity<I> + Copy> Quiddity<I> for core::cell::Cell<T> {
    describe!(|cx| cx.describe::<T, I>()?);

    fn encode(&self, e: &mut Encoder) {
        self.get().encode(e);
    }
}

/// The loaded value. Loading never blocks or panics.
macro_rules! atomics {
    ($($width:literal: $($ty:ident)*;)*) => {$($(
        #[cfg(target_has_atomic = $width)]
        impl<I: IdentityMarker> Quiddity<I> for core::sync::atomic::$ty {
            describe!(|cx| if stringify!($ty) == "AtomicBool" { Kd::Bool } else { Kd::Integer });

            fn encode(&self, e: &mut Encoder) {
                Quiddity::<I>::encode(&self.load(core::sync::atomic::Ordering::Relaxed), e);
            }
        }
    )*)*};
}

atomics! {
    "8": AtomicBool AtomicI8 AtomicU8;
    "16": AtomicI16 AtomicU16;
    "32": AtomicI32 AtomicU32;
    "64": AtomicI64 AtomicU64;
    "ptr": AtomicIsize AtomicUsize;
}

/// `{"Ok": x}` or `{"Err": e}`, as serde writes it in JSON.
impl<I: IdentityMarker, T: Quiddity<I>, E: Quiddity<I>> Quiddity<I> for Result<T, E> {
    describe!(|cx| X::tagged(
        alloc::vec![
            X::variant("Err", Some(cx.describe::<E, I>()?), None),
            X::variant("Ok", Some(cx.describe::<T, I>()?), None)
        ],
        None,
        None,
        None,
        false
    ));

    fn encode(&self, e: &mut Encoder) {
        let mut f = e.fields();
        match self {
            Ok(x) => f.field_as::<I, T>("Ok", x),
            Err(x) => f.field_as::<I, E>("Err", x),
        }
    }
}

impl<I: IdentityMarker> Quiddity<I> for core::convert::Infallible {
    fn encode(&self, _: &mut Encoder) {
        match *self {}
    }
}

/// Ranges are maps of their bounds, as serde writes them in JSON (spec 6.2).
mod ranges {
    use core::ops::{Bound, Range, RangeFrom, RangeInclusive, RangeTo, RangeToInclusive};

    use crate::{Encoder, IdentityMarker, Quiddity};

    impl<I: IdentityMarker, T: Quiddity<I>> Quiddity<I> for Range<T> {
        describe!(|cx| X::object(
            alloc::vec![
                ("end", X::field(cx.describe::<T, I>()?, false, false)),
                ("start", X::field(cx.describe::<T, I>()?, false, false))
            ],
            false
        ));

        fn encode(&self, e: &mut Encoder) {
            let mut f = e.fields();
            f.field_as::<I, T>("end", &self.end);
            f.field_as::<I, T>("start", &self.start);
        }
    }

    impl<I: IdentityMarker, T: Quiddity<I>> Quiddity<I> for RangeInclusive<T> {
        describe!(|cx| X::object(
            alloc::vec![
                ("end", X::field(cx.describe::<T, I>()?, false, false)),
                ("start", X::field(cx.describe::<T, I>()?, false, false))
            ],
            false
        ));

        fn encode(&self, e: &mut Encoder) {
            let mut f = e.fields();
            f.field_as::<I, T>("end", self.end());
            f.field_as::<I, T>("start", self.start());
        }
    }

    impl<I: IdentityMarker, T: Quiddity<I>> Quiddity<I> for RangeFrom<T> {
        describe!(|cx| X::object(
            alloc::vec![("start", X::field(cx.describe::<T, I>()?, false, false))],
            false
        ));

        fn encode(&self, e: &mut Encoder) {
            e.fields().field_as::<I, T>("start", &self.start);
        }
    }

    impl<I: IdentityMarker, T: Quiddity<I>> Quiddity<I> for RangeTo<T> {
        describe!(|cx| X::object(
            alloc::vec![("end", X::field(cx.describe::<T, I>()?, false, false))],
            false
        ));

        fn encode(&self, e: &mut Encoder) {
            e.fields().field_as::<I, T>("end", &self.end);
        }
    }

    impl<I: IdentityMarker, T: Quiddity<I>> Quiddity<I> for RangeToInclusive<T> {
        describe!(|cx| X::object(
            alloc::vec![("end", X::field(cx.describe::<T, I>()?, false, false))],
            false
        ));

        fn encode(&self, e: &mut Encoder) {
            e.fields().field_as::<I, T>("end", &self.end);
        }
    }

    impl<I: IdentityMarker, T: Quiddity<I>> Quiddity<I> for Bound<T> {
        describe!(|cx| X::tagged(
            alloc::vec![
                X::variant("Excluded", Some(cx.describe::<T, I>()?), None),
                X::variant("Included", Some(cx.describe::<T, I>()?), None),
                X::variant("Unbounded", None, None)
            ],
            None,
            None,
            None,
            false
        ));

        fn encode(&self, e: &mut Encoder) {
            match self {
                Bound::Included(x) => e.fields().field_as::<I, T>("Included", x),
                Bound::Excluded(x) => e.fields().field_as::<I, T>("Excluded", x),
                Bound::Unbounded => e.str("Unbounded"),
            }
        }
    }
}

impl<I: IdentityMarker> Quiddity<I> for core::time::Duration {
    describe!(|cx| Kd::Duration(Default::default()));

    /// Integer nanoseconds (spec 2.6).
    fn encode(&self, e: &mut Encoder) {
        e.int(self.as_nanos());
    }
}

/// IP and socket addresses are strings of their `Display` forms (spec 6.2).
macro_rules! addresses {
    ($($ty:ident)*) => {$(
        impl<I: IdentityMarker> Quiddity<I> for core::net::$ty {
            describe!(|cx| Kd::String);

            fn encode(&self, e: &mut Encoder) {
                e.display(self);
            }
        }
    )*};
}

addresses!(IpAddr Ipv4Addr Ipv6Addr SocketAddr SocketAddrV4 SocketAddrV6);

/// The bytes before the terminating NUL.
impl<I: IdentityMarker> Quiddity<I> for core::ffi::CStr {
    describe!(|cx| Kd::Bytes(Default::default()));

    fn encode(&self, e: &mut Encoder) {
        e.bytes(self.to_bytes());
    }
}

#[cfg(feature = "alloc")]
pub(crate) mod alloc_impls {
    use alloc::collections::{BTreeMap, BTreeSet, LinkedList, VecDeque};
    use alloc::ffi::CString;
    use alloc::string::String;
    use alloc::vec::Vec;

    use super::list;
    use crate::{Encoder, IdentityMarker, Quiddity};

    impl<I: IdentityMarker> Quiddity<I> for String {
        describe!(|cx| Kd::String);

        fn encode(&self, e: &mut Encoder) {
            e.str(self);
        }
    }

    impl<I: IdentityMarker, T: Quiddity<I>> Quiddity<I> for Vec<T> {
        describe!(|cx| Kd::list(cx.describe::<T, I>()?));

        fn encode(&self, e: &mut Encoder) {
            list::<I, T>(e, self);
        }
    }

    impl<I: IdentityMarker, T: Quiddity<I>> Quiddity<I> for VecDeque<T> {
        describe!(|cx| Kd::list(cx.describe::<T, I>()?));

        fn encode(&self, e: &mut Encoder) {
            list::<I, T>(e, self);
        }
    }

    impl<I: IdentityMarker, T: Quiddity<I>> Quiddity<I> for LinkedList<T> {
        describe!(|cx| Kd::list(cx.describe::<T, I>()?));

        fn encode(&self, e: &mut Encoder) {
            list::<I, T>(e, self);
        }
    }

    impl<I: IdentityMarker> Quiddity<I> for CString {
        describe!(|cx| Kd::Bytes(Default::default()));

        fn encode(&self, e: &mut Encoder) {
            e.bytes(self.as_bytes());
        }
    }

    impl<I: IdentityMarker, T: Quiddity<I>> Quiddity<I> for BTreeSet<T> {
        describe!(|cx| Kd::set(cx.describe::<T, I>()?));

        fn encode(&self, e: &mut Encoder) {
            set::<I, T>(e, self);
        }
    }

    impl<I: IdentityMarker, K: Quiddity<I>, V: Quiddity<I>> Quiddity<I> for BTreeMap<K, V> {
        describe!(|cx| X::map(cx.describe::<K, I>()?, cx.describe::<V, I>()?));
        flatten!(|this, m| for (k, v) in this {
            m.entry_as::<I, K, V>(k, v);
        });

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

    /// Encodes the items of a sequence as a set, under their primary
    /// identity: the derive's `encode_as = set`.
    pub fn encode_set<'a, C, T>(items: &'a C, e: &mut Encoder)
    where
        C: ?Sized,
        &'a C: IntoIterator<Item = &'a T>,
        T: Quiddity + 'a,
    {
        set::<crate::Primary, T>(e, items);
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
    use std::ffi::{OsStr, OsString};
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::alloc_impls::{map, set};
    use crate::{Encoder, IdentityMarker, Quiddity};

    impl<I: IdentityMarker, T: Quiddity<I>, S> Quiddity<I> for HashSet<T, S> {
        describe!(|cx| Kd::set(cx.describe::<T, I>()?));

        fn encode(&self, e: &mut Encoder) {
            set::<I, T>(e, self);
        }
    }

    impl<I: IdentityMarker, K: Quiddity<I>, V: Quiddity<I>, S> Quiddity<I> for HashMap<K, V, S> {
        describe!(|cx| X::map(cx.describe::<K, I>()?, cx.describe::<V, I>()?));
        flatten!(|this, m| for (k, v) in this {
            m.entry_as::<I, K, V>(k, v);
        });

        fn encode(&self, e: &mut Encoder) {
            map::<I, K, V>(e, self);
        }
    }

    impl<I: IdentityMarker> Quiddity<I> for SystemTime {
        describe!(|cx| Kd::Instant(Default::default()));

        /// Integer nanoseconds since the Unix epoch, negative before it
        /// (spec 2.6).
        fn encode(&self, e: &mut Encoder) {
            // A `Duration` holds less than 2^94 nanoseconds, so an `i128`
            // holds it negated too.
            match self.duration_since(UNIX_EPOCH) {
                Ok(after) => e.int(after.as_nanos()),
                Err(before) => e.int(-(before.duration().as_nanos() as i128)),
            }
        }
    }

    /// The platform's bytes for the string: as they are on Unix, and WTF-8
    /// on Windows (spec 6.2).
    impl<I: IdentityMarker> Quiddity<I> for OsStr {
        describe!(|cx| Kd::Path);

        fn encode(&self, e: &mut Encoder) {
            #[cfg(unix)]
            e.bytes(std::os::unix::ffi::OsStrExt::as_bytes(self));
            #[cfg(windows)]
            e.utf16(std::os::windows::ffi::OsStrExt::encode_wide(self));
            #[cfg(not(any(unix, windows)))]
            e.bytes(self.as_encoded_bytes());
        }
    }

    impl<I: IdentityMarker> Quiddity<I> for OsString {
        describe!(|cx| Kd::Path);

        fn encode(&self, e: &mut Encoder) {
            Quiddity::<I>::encode(self.as_os_str(), e);
        }
    }

    impl<I: IdentityMarker> Quiddity<I> for Path {
        describe!(|cx| Kd::Path);

        fn encode(&self, e: &mut Encoder) {
            Quiddity::<I>::encode(self.as_os_str(), e);
        }
    }

    impl<I: IdentityMarker> Quiddity<I> for PathBuf {
        describe!(|cx| Kd::Path);

        fn encode(&self, e: &mut Encoder) {
            Quiddity::<I>::encode(self.as_os_str(), e);
        }
    }
}

/// A digest used as a value is the string of its text form (spec 6.2).
macro_rules! digests {
    ($($t:ty)*) => {$(
        impl<I: IdentityMarker> Quiddity<I> for $t {
            describe!(|cx| Kd::String);

            fn encode(&self, e: &mut Encoder) {
                e.display(self);
            }
        }
    )*};
}

digests!(Digest Digest128 Digest64);
