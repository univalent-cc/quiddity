//! Compile-fail tests that need a full build, not `cargo check` (which the
//! trybuild tests in `tests/ui` use): errors that appear only once generic
//! code is instantiated.

/// `transparent` on a generic struct whose field turns out to be
/// `PhantomData`, which does not participate:
///
/// ```compile_fail,E0080
/// use std::marker::PhantomData;
/// use quiddity::{Quiddity, QuiddityExt};
///
/// #[derive(Quiddity)]
/// #[quiddity(transparent)]
/// struct Wrapper<T> {
///     value: T,
/// }
///
/// let w: Wrapper<PhantomData<u8>> = Wrapper { value: PhantomData };
/// let _ = w.quiddity();
/// ```
///
/// The same with a field that participates compiles:
///
/// ```
/// use quiddity::{Quiddity, QuiddityExt};
///
/// #[derive(Quiddity)]
/// #[quiddity(transparent)]
/// struct Wrapper<T> {
///     value: T,
/// }
///
/// assert_eq!(Wrapper { value: 5u8 }.quiddity(), 5u8.quiddity());
/// ```
pub struct TransparentPhantom;
