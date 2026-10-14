//! `IsEmpty` and `Nullable`, used by the derive's `omit_empty` and
//! `omit_none`: whether a value is an empty string, list, set or map, and
//! whether it is null (spec 5.5).

/// Whether a value is empty. Implemented for the types that encode as a
/// string, list, set or map, so that `omit_empty` on any other type fails to
/// compile.
#[diagnostic::on_unimplemented(
    message = "`omit_empty` applies only to strings, sequences, sets and maps, not `{Self}`",
    label = "cannot be empty"
)]
pub trait IsEmpty {
    /// Whether the value is empty.
    fn is_empty(&self) -> bool;
}

macro_rules! by_len {
    ($($(#[$cfg:meta])* [$($g:tt)*] $ty:ty;)*) => {$(
        $(#[$cfg])*
        impl<$($g)*> IsEmpty for $ty {
            fn is_empty(&self) -> bool {
                <$ty>::is_empty(self)
            }
        }
    )*};
}

by_len! {
    [] str;
    [T] [T];
    [] core::ffi::CStr;
    #[cfg(feature = "alloc")]
    [] alloc::string::String;
    #[cfg(feature = "alloc")]
    [T] alloc::vec::Vec<T>;
    #[cfg(feature = "alloc")]
    [T] alloc::collections::VecDeque<T>;
    #[cfg(feature = "alloc")]
    [T] alloc::collections::LinkedList<T>;
    #[cfg(feature = "alloc")]
    [T] alloc::collections::BTreeSet<T>;
    #[cfg(feature = "alloc")]
    [K, V] alloc::collections::BTreeMap<K, V>;
    #[cfg(feature = "std")]
    [T, S] std::collections::HashSet<T, S>;
    #[cfg(feature = "std")]
    [K, V, S] std::collections::HashMap<K, V, S>;
    #[cfg(feature = "std")]
    [] std::ffi::OsStr;
    #[cfg(feature = "serde_json")]
    [] serde_json::Map<alloc::string::String, serde_json::Value>;
    #[cfg(feature = "indexmap")]
    [K, V, S] indexmap::IndexMap<K, V, S>;
    #[cfg(feature = "indexmap")]
    [T, S] indexmap::IndexSet<T, S>;
    #[cfg(feature = "bytes")]
    [] bytes::Bytes;
    #[cfg(feature = "bytes")]
    [] bytes::BytesMut;
}

// Types without an inherent `is_empty`, for which `<T>::is_empty` would
// call this trait's method.

#[cfg(feature = "alloc")]
impl IsEmpty for alloc::ffi::CString {
    fn is_empty(&self) -> bool {
        self.as_bytes().is_empty()
    }
}

#[cfg(feature = "std")]
impl IsEmpty for std::ffi::OsString {
    fn is_empty(&self) -> bool {
        self.as_os_str().is_empty()
    }
}

#[cfg(feature = "std")]
impl IsEmpty for std::path::Path {
    fn is_empty(&self) -> bool {
        self.as_os_str().is_empty()
    }
}

#[cfg(feature = "std")]
impl IsEmpty for std::path::PathBuf {
    fn is_empty(&self) -> bool {
        self.as_os_str().is_empty()
    }
}

impl<T, const N: usize> IsEmpty for [T; N] {
    fn is_empty(&self) -> bool {
        N == 0
    }
}

/// `Some(x)` is empty when `x` is. `None` is null, which is not empty; use
/// `omit_none` to omit it too.
impl<T: IsEmpty> IsEmpty for Option<T> {
    fn is_empty(&self) -> bool {
        self.as_ref().is_some_and(T::is_empty)
    }
}

/// Pointers are empty when what they point to is.
macro_rules! pointers {
    ($($(#[$cfg:meta])* $ty:ty;)*) => {$(
        $(#[$cfg])*
        impl<T: IsEmpty + ?Sized> IsEmpty for $ty {
            fn is_empty(&self) -> bool {
                T::is_empty(self)
            }
        }
    )*};
}

pointers! {
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
impl<T: IsEmpty + alloc::borrow::ToOwned + ?Sized> IsEmpty for alloc::borrow::Cow<'_, T> {
    fn is_empty(&self) -> bool {
        T::is_empty(self)
    }
}

/// A JSON string, array or object with nothing in it. Other values,
/// including `null`, are not empty.
#[cfg(feature = "serde_json")]
impl IsEmpty for serde_json::Value {
    fn is_empty(&self) -> bool {
        use serde_json::Value;
        match self {
            Value::String(s) => s.is_empty(),
            Value::Array(a) => a.is_empty(),
            Value::Object(m) => m.is_empty(),
            Value::Null | Value::Bool(_) | Value::Number(_) => false,
        }
    }
}

/// Whether a value is null. Implemented for the types that can encode as
/// null, so that `omit_none` on any other type fails to compile.
#[diagnostic::on_unimplemented(
    message = "`omit_none` applies only to types that can be null \
               (`Option`, `()`, `serde_json::Value`, …), not `{Self}`",
    label = "cannot be null"
)]
pub trait Nullable {
    /// Whether the value is null.
    fn is_null(&self) -> bool;
}

/// `None`. `Some(x)` is not null even when `x` encodes as null, as with
/// `Some(None)`.
impl<T> Nullable for Option<T> {
    fn is_null(&self) -> bool {
        self.is_none()
    }
}

impl Nullable for () {
    fn is_null(&self) -> bool {
        true
    }
}

#[cfg(feature = "serde_json")]
impl Nullable for serde_json::Value {
    fn is_null(&self) -> bool {
        serde_json::Value::is_null(self)
    }
}

/// Pointers are null when what they point to is.
macro_rules! nullable_pointers {
    ($($(#[$cfg:meta])* $ty:ty;)*) => {$(
        $(#[$cfg])*
        impl<T: Nullable + ?Sized> Nullable for $ty {
            fn is_null(&self) -> bool {
                T::is_null(self)
            }
        }
    )*};
}

nullable_pointers! {
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
impl<T: Nullable + alloc::borrow::ToOwned + ?Sized> Nullable for alloc::borrow::Cow<'_, T> {
    fn is_null(&self) -> bool {
        T::is_null(self)
    }
}
