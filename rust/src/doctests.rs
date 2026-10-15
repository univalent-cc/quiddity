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

/// `flatten` on a field whose type has no members to flatten (D21):
///
/// ```compile_fail,E0080
/// use quiddity::{Quiddity, QuiddityExt};
///
/// #[derive(Quiddity)]
/// struct Query {
///     term: String,
///     #[quiddity(flatten)]
///     page: u32,
/// }
///
/// let _ = Query { term: "a".into(), page: 1 }.quiddity();
/// ```
///
/// The same with a struct compiles:
///
/// ```
/// use quiddity::{Quiddity, QuiddityExt};
///
/// #[derive(Quiddity)]
/// struct Page {
///     page: u32,
/// }
///
/// #[derive(Quiddity)]
/// struct Query {
///     term: String,
///     #[quiddity(flatten)]
///     page: Page,
/// }
///
/// let _ = Query { term: "a".into(), page: Page { page: 1 } }.quiddity();
/// ```
#[cfg(feature = "alloc")]
pub struct FlattenScalar;
