# quiddity — design notes

This file records design decisions, the alternatives considered, and
constraints on the implementation. The README describes behaviour; this file
records why, and what the implementation must honour that the README does
not mention.

**Status convention.** Everything the README states before its
`## Attributes` heading is settled. Everything from that heading on is still
open to change, as are the open items and desiderata below.

## Origin

`quiddity` began as a design question in cog-gateway (`HUMAN-DESIGN.md`,
H-3). A human cogitator must check that an answer was given to the question
currently asked, not an earlier version of it with the same tag. The proposed
check was a content digest over the fields that define the question (brief,
questions, rows, document digests), excluding those that do not (tags,
priority, limits, routing), sent back by the client as an HTTP precondition
(`If-Match`, `412 Precondition Failed`).

Existing code computes such digests by hand. In the ULC project, a
provision's checksum is SHA-256 over its text alone, excluding its label,
address and span, so that renumbering does not read as a change of
substance. Which fields count is known only to a hand-written function. The
aim of `quiddity` is to declare it on the type instead.

## Goal: implementable in other languages

Implementations in other languages are out of scope for the first version,
but the design should not prevent them. A digest must be computable from a
value's data and a set of strings, without reference to Rust. In practice:

- Everything that enters a digest is defined in `SPEC.md` in terms of a
  language-neutral data model (null, boolean, number, string, bytes, list,
  set, map), not Rust types. Rust-specific forms (`Option`, tuples, structs,
  enums) are defined by how they map onto it.
- Labels are strings: domains (D4) and field names after
  `rename`. Defaults derived from Rust identifiers are acceptable, because
  only the resulting string enters the digest.
- Implementation constraints, such as not allocating (below), belong to the
  Rust implementation, not to the format. Format choices should be easy to
  implement elsewhere; this bore on D13.

## D1. Write a new crate

The README's Alternatives section surveys the existing crates. The deciding
points:

- No crate combines field selection on the type, a specified stable
  encoding, and tolerance of schema changes.
- `borsh` came closest: field selection with `#[borsh(skip)]` (independent
  of serde), a specification, maintained. It was rejected because:
  - an impl for `serde_json::Value` is a hard requirement. borsh's
    maintainers declined one (near/borsh-rs#312) in favour of an example
    (#317), and the orphan rule prevents adding it outside borsh, so every
    `Value` field would need `serialize_with` or a newtype;
  - one choice of fields per type, with no named identities;
  - positional encoding, so adding a field changes every digest.
- `digestible` has the intended API, but its encoding is not stable
  (iteration-order maps, no length prefixes, untagged variants). Fixing it
  would replace the encoding and change every signature, i.e. a rewrite of a
  single-author crate.

## D2. Name

`quiddity`: the scholastic term for what makes a thing what it is, as
distinct from its incidental properties. `identity-hash` was rejected: the
crate name is taken, and "identity hasher" already means a pass-through
`Hasher` in Rust (`identity-hash`, `nohash-hasher`).

## D3. Identities are selected by marker types

An identity is a zero-sized type deriving `IdentityMarker`, and a type with
identity `I` implements `Quiddity<I>`. Call sites write
`value.quiddity_as::<I>()`.

A marker has no name of its own. Identities are not hashed (D4), and a
digest cannot be inspected for the identity that produced it, so a name
would have no use in a digest. Where a marker needs to be displayed, its
`Debug` impl uses the Rust type's name.

Alternatives considered:

- **A′: the identity as a value whose type drives dispatch**
  (`value.quiddity(&Meaning)`), which would allow parameterized identities.
- **String names** (`quiddity_as("meaning")`): typos become run-time errors,
  and nothing checks that a type supports an identity.
- **Generated methods** (`meaning_quiddity()`): the identity cannot be a
  generic parameter.
- **Per-type enums** (`TaskIdentity::Meaning`): an identity is not one thing
  across types, so nested values need a mapping at every level.
- **Run-time identity constants** (`const MEANING: Identity`): an identity
  passes naturally to nested values, but "this type does not support this
  identity" becomes a run-time fallback or error.

Marker types were chosen because the compiler checks that a type supports an
identity, and an identity can be a generic parameter
(`fn f<I: IdentityMarker, T: Quiddity<I>>`). Accepted costs:

- Identities are not parameterized.
- A nested value is digested under its own primary identity unless its field
  has `identity = I`. Decided over passing the enclosing identity down (which
  would make a nested type lacking that identity a compile error unless its
  field says `identity = Primary`): the chosen rule always compiles, and a
  trait bound cannot express "the same identity if the nested type has it,
  otherwise its primary one" without specialization.

A further option, listing each identity's fields on the container
(`identity(meaning = [prompt, inputs])`) instead of annotating fields, is
independent of the selection mechanism and was not adopted.

## D4. Labels: identities are not hashed; domains are opt-in

**Identities do not enter the hash.** An identity exists to give
instructions for building the encoding: which fields, under which names,
with which parameters. It already affects the digest through those. Two
identities that produce the same encoding for a value are, for that value,
the same, and give the same digest. So a `Task` digested under `Meaning`
equals the digest of a JSON object with the same fields and values.

**A domain is hashed in only when set.** A type has no domain unless
`domain = "…"` is set. With one, the digest is computed in two steps:

```
structural = SHA-256( structural tag ‖ version ‖ encoding of the value )
in domain  = SHA-256( domain tag ‖ version ‖ length ‖ domain label ‖ structural )
```

The two inputs start with different tags, so a structural digest can never
equal an in-domain one by construction, and the length prefix keeps one
label from being a prefix of another.

**Domains apply to the top-level digest only.** A nested value contributes
its encoding, never its digest, so its own type's domain plays no part.
This needs no code: the domain step happens in `quiddity_as`, which nested
values never pass through. One mechanism, no special case.

Known limitation: a domain protects the digest of a value of its type, not
of values containing one. `Envelope<Payment>` and `Envelope<Refund>` with the
same fields have the same digest, even with a domain on `Envelope`. An enum
field (`Action::Payment(..)`) does distinguish them, since the variant name
is encoded. Rejected alternatives, both of which close the gap:

- An inline domain marker before each nested value whose type has a domain,
  with the two-step form kept at the top level: two mechanisms for one
  concept.
- An inline marker everywhere, top level included: one mechanism, but a
  digest could then no longer be raised into a domain from the digest
  alone.

Consequences:

- A type without a domain pays nothing; a type with one pays one extra
  SHA-256 block (64 bytes) per digest.
- A `Digest` is the hash bytes and nothing else (W2 is met trivially).
- A structural digest can be raised into a domain from the digest alone,
  without the value: `digest.in_domain("example.task")`. This is how a
  struct with a domain is compared with untyped data: digest the JSON value,
  then raise it into the struct's domain.
- Raising is one-way. An in-domain digest cannot be lowered, and nothing in
  its bytes says whether it is in a domain, or which. There is no API for
  the structural digest of a type that has a domain: a type is given a
  domain because its digests belong in it, and comparing it with untyped
  data raises the other side instead.

**What a domain is for.** There are two kinds of collision:

1. A hash collision: different encodings with the same SHA-256. This is
   negligible and the domain does nothing about it.
2. An encoding collision: different values with the same encoding, which
   therefore always have the same digest. Across types, values with the same
   field names and values encode identically, because the encoding ignores
   Rust types. This is intended: it is what makes a struct comparable with
   a map, a JSON value or another representation of the same data.

A domain makes a digest commit to the kind of thing it was computed from. It
matters in one situation: when digests are signed or authorized by someone
holding a secret key. If Alice signs the digest of `Payment { to: Bob,
amount: 100 }`, and `Refund { to: Bob, amount: 100 }` has the same digest,
Bob can present the refund with Alice's signature, and it verifies. Bob
forges nothing; he reuses a real signature under another meaning. With
domains, the refund's digest differs and the signature does not verify.
This is the purpose of EIP-712's domain separator and Aptos's per-type BCS
salts. Secondarily, a domain keeps types apart in a store keyed by digest,
which is a matter of avoiding mistakes, not attacks.

`quiddity` provides no secrets and no resistance to forgery. Anyone can
compute any digest, in any domain; the encoding is public. That is why
raising a digest into a domain is harmless: the protection in the case above
comes from the signer's key, and a domain only fixes what the signature
means.

**Why not default to the type name.** It separates types that should match
(`Task` and `TaskRef`), changes digests when a type is renamed or moved
(contradicting stability; `std::any::type_name` is not even stable across
compilers), and protects only authors who have not considered the question.

**Rejected: carrying labels in the `Digest`.** A `Digest` holding its
domain (and identity) beside the hash would allow introspection and
comparison of digests from unlike types after the fact, but costs space in
every digest, an interning registry to deserialize labels without
allocating (`&'static str` literals are not guaranteed one address each, so
comparison would also need a byte-comparison fallback), and gives no
protection to signatures unless the label is hashed in anyway. The
structural digest plus raising covers the use case of comparing a struct
with JSON, because whoever knows the JSON is meant to be a `Task` can raise
its digest into `Task`'s domain.

## D5. Encoding decisions

- **Fields are keyed by name and sorted, by default** (see D6). Field order
  does not need to be free (it is acceptable for it to affect a digest), but
  keying by name makes it free, and keying by name is what allows `rename`
  and adding fields without changing digests.
- **Absent is not null.** A `None` field hashes as null, as JSON's `null`
  does. With `none_as_missing` it hashes as a missing key instead, so adding
  the field is not a change. Empty collections are not omitted
  automatically; other values are omitted only with `omit_if = path`
  (`stable-hash` omits all defaults).
- **Numbers are encoded by value** (D14). Integer width does not matter, a
  float exactly equal to an integer encodes as that integer, `-0.0` as `0`,
  and all NaNs alike, so computing a digest cannot fail.
- **JSON object keys are unordered.** RFC 8259 defines objects as unordered.
  serde_json's `preserve_order` feature changes `Map` iteration order, and
  Cargo unifies features across the dependency graph, so any crate can turn
  it on; the digest must not depend on it.
- **No Unicode normalization.** Normalizing would make distinct strings
  equal and add a dependency on Unicode tables. Documented, and left to the
  caller.
- **SHA-256 only**, displayed as `sha256:<hex>` (the same form ULC uses).

## D6. Shapes: `encode_as`

Every value is encoded as a shape of the data model (null, boolean, number,
string, bytes, list, set, map). Rust types map onto shapes as serde maps
them onto JSON by default, so a value's digest equals that of its serde JSON
form:

| Rust | Default shape | `encode_as` alternative |
|---|---|---|
| Struct with named fields | map: field name to value | `list` |
| Tuple struct | list | none (no names for a map) |
| Newtype `struct Id(T)` | its single field | none |
| Unit struct | null | none |
| Enum, unit variant | string: the variant's name | `int` |
| Enum, variant with data | map: `{name: data}` (serde's external tagging) | `int` |

**Naming.** The attribute is a single valued one, `encode_as = <shape>`,
rather than one flag per shape (`hash_as_list`, …): the default can be
written explicitly, which reads better when scoped (`encode_as = list,
encode_as(Meaning) = map`), and contradictory flags cannot be written.
"encode", not "hash" or "digest": a nested value is never digested; the
attribute decides the shape of its encoding, which matches `Quiddity::encode`
and `Encoder`. `transparent` is kept for single-named-field structs, as in
serde.

**`encode_as = display`.** Any type implementing `Display` can be encoded as
its `Display` output, a string: on a type definition (derive), or on a
field, where it is the easy route for foreign types (the orphan rule). It
is a separate value, not an extension of `string`, because `string` on an
enum already means "the variant's name", which differs from the enum's
`Display` output. The output is streamed into the hasher as it is
formatted, and its length written after it (D14), so it is formatted once,
without allocating. `encode_as` is
otherwise a type-level attribute; `display` and `serde` (D11) are the
values allowed on fields.

**Struct as map (default).** Exactly the encoding of a map with string keys:
same tag, same ordering. A struct, a `HashMap<String, _>`, a
`BTreeMap<String, _>` and a JSON object with the same entries have the same
digest. This is what lets a Rust struct be compared with data from dynamic
languages.

**Struct as list.** Pure positional: every field, in declaration order,
with no names, so it equals a tuple or list of the same values. It is for
performance, with ordinary hashing behaviour and no guarantees: adding,
removing or reordering fields changes digests, and the struct no longer
equals a map. Renaming a field does not change digests.

A field id scheme (`#[quiddity(id = 3)]`, as in protobuf) was considered,
keeping omission and adding fields while dropping names, and rejected: the
map default already gives the schema-change guarantees, and the list shape
is for those who do not want them.

**Enum as string (default).** Matches serde's JSON, survives reordering
variants, and works with `rename`.

**Enum as int.** A variant's integer is its Rust discriminant: the value
written in the enum, or else one more than the previous variant's, starting
at 0. Writing discriminants explicitly makes reordering safe. A variant with
data is a two-element list `[discriminant, data]` (JSON has no integer map
keys, so `{3: data}` was rejected).

**`omit_if = path`.** Omits the field when `path(&value)` returns `true`,
as serde's `skip_serializing_if` does. For adding a field that is not an
`Option` (`retries: u32`, `tags: Vec<String>`) without changing existing
digests, and for matching JSON that leaves out empty or zero values. It
replaces an earlier `omit_default`, which would have built
`Default::default()` and compared with it: possibly allocating (against the
no-allocation constraint), needing `PartialEq`, and of unbounded cost. With
`omit_if` the user chooses the check and owns its cost.
`none_as_missing` is the special case `omit_if = Option::is_none`, kept as
its own attribute because it is the common case and pairs with JSON's null
versus a missing key.

**`none_as_missing`.** Without it, a `None` field hashes as JSON `null`
would. With it, `None` hashes as a missing key would. The derive cannot
check that a field is an `Option` by its tokens (aliases, paths and
re-exports defeat a syntactic check), so the attribute expands to a call
through a trait implemented for `Option<T>`; any other type fails to
compile. There is no std trait for "nullable".

Open questions:

- `none_as_missing` and `omit_if` under `encode_as = list` have no meaning
  (a position cannot be missing). Proposed: a compile error.

## D7. Trait structure

The primary identity, the one `quiddity()` uses, is a marker type like any
other, `Primary`, so `Quiddity<I>` is always parameterized. (Rejected names:
`Default`, which clashes with the prelude's `Default` trait; `Whole`, since
skipped fields are not part of it; `Base`, which suggests a minimal subset
that other identities build on.) Sketch:

```rust
pub trait IdentityMarker: 'static {}

/// The primary identity: the one `quiddity()` uses.
pub struct Primary;
impl IdentityMarker for Primary {}

/// How a value is encoded under identity `I`. Written by the derive, or by hand.
pub trait Quiddity<I: IdentityMarker = Primary> {
    /// A const, so a type without a domain pays nothing at run time.
    const DOMAIN: Option<&'static str> = None;

    fn encode(&self, e: &mut Encoder);
}

/// What callers use.
pub trait QuiddityExt {
    fn quiddity(&self) -> Digest
    where
        Self: Quiddity<Primary>,
    {
        self.quiddity_as::<Primary>()
    }

    fn quiddity_as<I: IdentityMarker>(&self) -> Digest
    where
        Self: Quiddity<I>,
    {
        let mut e = Encoder::new(); // wraps a SHA-256 state; writes the tag and version
        <Self as Quiddity<I>>::encode(self, &mut e);
        let structural = e.finish();
        match <Self as Quiddity<I>>::DOMAIN {
            None => structural,             // resolved at compile time
            Some(d) => structural.in_domain(d),
        }
    }
}
impl<T: ?Sized> QuiddityExt for T {}

// Leaves and containers implement every identity, so that `identity = I` works
// through `Vec`, `Option`, maps and so on.
impl<I: IdentityMarker> Quiddity<I> for u64 { /* … */ }
impl<I: IdentityMarker, T: Quiddity<I>> Quiddity<I> for Vec<T> { /* … */ }
```

The derive emits one `impl Quiddity<X> for T` per identity (`Primary` plus
each one listed in `identities(...)`), with that identity's fields sorted by
name at expansion time.

**Encoder API.** Primitives: `null`, `bool`, `int` (any primitive
integer, through the sealed `Integer` trait), `int_twos_complement` (an
integer of any size, as big-endian two's-complement bytes, the form
`num_bigint` produces), `float` (normalized), `str`, `bytes`, and `display`.
Streams: `str_stream()` (a `fmt::Write`) and `bytes_stream()` (an
`io::Write` with `std`), which write the length when they end. Composites:
`list()`, `set()` and `map()` (the last two need `alloc` and sort their
entries, D13), and `fields()`, a map with `&'static str` keys written in
canonical order without buffering, checked in debug builds. Each writer
has `item`/`member`/`entry`/`field`, an `_as::<I, _>` form, and a `_with`
form taking a closure that writes one value. A writer ends when it is
finished or dropped, so forgetting `finish()` cannot corrupt an encoding.

**Bounds in derived impls.** For a generic type, the derive adds
`T: Quiddity<J>` for each type parameter `T` that appears in a field written
through its own impl under identity `J`, as serde does for `Serialize`.
Type parameters inside `PhantomData<…>` get no bound, so `Id<T>` with a
`PhantomData<T>` field does not require `T: Quiddity`. Bounds on field
types themselves were rejected: they make recursive types (`struct Node {
children: Vec<Node> }`) overflow the trait solver.

**Why `QuiddityExt` is a separate trait.** The caller-facing methods must
live on a trait implemented once per type. A type implements `Quiddity<I>`
once per identity, so a method provided by `Quiddity<I>` exists once per
identity: `task.quiddity()` would fail with E0283 as soon as the type
declares a second identity (a trait's default type parameter takes no part
in inference), and the turbofish in `quiddity_as::<I>()` cannot reach a
parameter of the trait. A blanket-implemented `QuiddityExt` also means no
implementer can change how a digest is assembled (encoding, then domain);
Rust has no `final` methods.

## D8. Every attribute can be scoped to identities

Any attribute, on a type, field or variant, except `identities(…)`, can be
limited to identities by naming them in parentheses after the attribute:
`skip(Meaning)`, `rename(Meaning) = "question"`, `skip(Meaning, Routing)`.
This replaces the earlier `only(…)` and `except(…)` field attributes:
`except(Meaning)` is `skip(Meaning)`, and `only(Meaning)` is
`skip(Primary)` (plus any other named identities to leave it out of).

In scope lists the derive recognizes `Primary` by name, so it need not be
imported, and it means `quiddity::Primary` even if the user has another
type of that name in scope. A user marker type named `Primary` is rejected.

Rules:

- Identities do not inherit from one another; the primary identity is not
  a base for the others. An unscoped attribute applies to every identity,
  the primary one included. A scoped attribute applies to the identities it
  names, replacing the unscoped form for them. So `skip` on a timestamp
  leaves it out of every identity, and a `rename` is written once.
- Every attribute has the form `name[(Identity, …)] [= value]`. A flag
  written without a value means `= true`, so a scoped `= false` can undo an
  unscoped flag for one identity.

The `name(Identity) = value` form is not standard `syn::Meta` syntax, so the
derive needs its own attribute parser (`darling` will not parse it as is).

Open:

- Whether a scoped `domain` (a different domain per identity) is useful, or
  should be an error. Since identities are not hashed (D4), a scoped domain
  would be the way to make two identities' digests differ deliberately.

## D9. Shorter digests

`Digest::mini()` gives the first 128 bits (`Digest128`), and
`Digest::micro()` the first 64 bits (`Digest64`). The size is chosen where
the digest is used (an ETag, a key), not on the type. Shorter digests save
space, not time: SHA-256 is computed in full.

Rejected:

- A crate-wide feature flag. Cargo unifies features across the dependency
  graph, so one crate enabling it would change the digests of every crate
  using `quiddity` in that build; Cargo expects features to be additive. A
  `--cfg` set by the final application is still global.
- A per-type attribute (`digest_bits = 128`). It ties the size to the type,
  while different uses of one type want different sizes. Not needed given
  the methods; can be added later as sugar (an associated type on
  `Quiddity<I>`, since an associated const cannot be a const generic
  argument on stable Rust).

Details:

- A shortened structural digest is a prefix of the full one.
- `in_domain` is defined on the full `Digest` only: domain first, then
  shorten. This keeps one rule, and makes `x.in_domain(d).mini()` the only
  way to get a short in-domain digest, so short digests always agree.
- Text forms state the size: `sha256-128:`, `sha256-64:`.
- 64 bits suits comparing a known pair, not large keyspaces (birthday bound
  about 2^32) or adversarial uses (D4).
- Names: `mini`/`micro` chosen; `half`/`quarter` and `to_128`/`to_64` were
  alternatives.

## D10. Optional features for other crates

Criteria for a built-in impl behind a feature:

1. Users put the type in their own structs, where the orphan rule would
   otherwise force `with` or `encode_as = display` on every field.
2. Its values map onto the data model without a judgment call.
3. Its API is stable. Each optional dependency ties `quiddity` to that
   crate's major versions: a new major version needs a new feature, and an
   old one cannot be removed without a breaking release.

Shipped: `serde_json`, `indexmap` (`IndexMap`, `IndexSet`), `uuid` (the
hyphenated lowercase string, as serde writes it in JSON, so a `Uuid` equals
its JSON form; 16 bytes via `with`), `bytes` (`Bytes`, `BytesMut` as the
bytes kind; the `bytes` crate is maintained by the tokio project but is a
separate crate), `url` (its string form).

`Bytes` is the bytes kind while `Vec<u8>` is a list of numbers: `Vec<u8>` is
an instance of `Vec<T>`, and its serde JSON form is an array, while `Bytes`
exists to hold binary data. So a `Bytes` does not equal the JSON array a
serializer writes for it.

Deferred until requested (the README invites issues and pull requests;
meanwhile the `serde` feature, D11, covers most of them):

- Time types (`chrono`, `time`, `jiff`): each needs a representation choice
  (RFC 3339 string or epoch number, precision, time zones). Following each
  crate's serde representation is the natural rule.
- `rust_decimal`: exact decimals are neither integers nor floats in the
  data model.
- Other formats' value types (`toml::Value`, `serde_yaml::Value`,
  `ciborium::Value`, `rmpv::Value`, `bson::Bson`): each has kinds outside
  the data model (BSON's `ObjectId`, `Decimal128`, datetimes; YAML's
  non-string keys and tags; TOML's datetimes) needing mapping decisions.
- `smallvec`, `arrayvec`: plain sequences, cheap to add.

Out of scope: `ndarray`, `polars`. The mapping is ambiguous, the
dependencies heavy (Polars breaks its API often), and a per-value tagged
encoding is the wrong tool for millions of elements; users hash raw buffers.

## D11. The `serde` feature

Behind its own `serde` feature, separate from `serde_json`. A direct
implementation of `serde::Serializer` that writes into the `Encoder`, not a
detour through `serde_json::Value`. The detour would build the whole value
in memory before hashing, and would digest its JSON rendering rather than
the value: bytes would become arrays, NaN and the infinities would be
rejected, integers wider than 64 bits would fail without serde_json's
`arbitrary_precision`, and non-string map keys would be stringified or
rejected.

The one `serde` feature (requiring `alloc`) also provides `Serialize` and
`Deserialize` for `Digest`, `Digest128` and `Digest64`, as their text forms.
A separate feature for the serializer was considered and rejected: it would
only serve `no_std` users without an allocator who serialize digests.

API: `encode_as = serde`, on a type or a field, available only with the
feature; `quiddity::serde::digest(&value) -> Result<Digest, Error>` for any
`Serialize` value; and `quiddity::serde::encode(&value, &mut Encoder)` for
manual impls.

Rules:

- serde's data model follows the D6 shapes: unit as null, newtype as its
  inner value, tuple and sequence as a list, struct and map as a map, unit
  variant as its name, other variants as `{name: data}`. A `Serialize`
  value must encode exactly as the equivalent native or derived value;
  test this across the std impls.
- `serialize_bytes` is the bytes kind; `i128`/`u128` are integers; a field
  left out by `skip_serializing_if` is a missing key.
- Errors: `Serialize` impls may fail (a mutably borrowed `RefCell`, a
  poisoned `Mutex`, a custom impl or `serialize_with` that returns an
  error). `digest` returns the error. Through `encode_as = serde` or
  `encode`, the failure happens inside the infallible `quiddity()` /
  `quiddity_as()`, which panic; documented. A recorded error with
  `try_quiddity()` was considered and rejected as over-built for a rare
  case.
- Allocation: serde delivers map entries and struct fields once, in its
  own order, so they are sorted into a buffer, as for any unordered map
  (D13). Sequences of unknown length need no buffer, because counts are
  written after the items (D14).

## D12. `no_std` tiers: `std` and `alloc` features

`default = ["std", "derive"]`, and `std` enables `alloc`, so all three are on by
default. `derive` re-exports the macros from `quiddity-derive`; turning it
off avoids compiling the proc macro, for crates with only manual impls.
With default features off, the crate is `no_std` and needs no allocator.
This costs little because hashing never allocates (see the constraint
below), and it serves firmware or embedded code that verifies signed
records by digest, bootloaders and kernels, and some WebAssembly targets.
It is a CI check (a build for a `no_std` target), not a headline feature.

- Core only: the `Encoder` (the `sha2` crate is `no_std`), `Digest`,
  `Digest128`, `Digest64` and their text forms (`core::fmt`), the derive
  and traits, integers, floats, `bool`, `char`, `&str`, slices, arrays,
  tuples, `Option`, references, and `encode_as = display` (two-pass
  formatting needs only `core::fmt`).
- `alloc`: `String`, `Vec`, `VecDeque`, `Box`, `Rc`, `Arc`, `Cow`,
  `BTreeMap`, `BTreeSet`. Required by `serde_json` (which supports `no_std`
  with `alloc`), `serde` (the bridge buffers, D11), `indexmap` and `bytes`.
- `std`: `HashMap` and `HashSet`. Required by `url` (assumed; recent `url`
  versions may support `no_std`, to be checked). `uuid` needs neither.

## Guideline: hashing avoids allocation

Hashing should not allocate without good reason: no buffering out of
convenience. The `Encoder` streams into a fixed-size SHA-256 state. The
accepted exception is sorting an unordered map or set (D13), a different
order of cost that needs a buffer; every unordered map type needs `alloc`
anyway, so the `no_std` core (D12) is unaffected. Otherwise:

- Struct fields are sorted at macro-expansion time, not at run time.
- Lengths and counts are written after the items they describe (D14), so
  nothing needs to be known in advance: not a stream's length, not a
  `Display` output's length, not how many fields `omit_if` and
  `none_as_missing` leave out.

## D13. Canonical order of map entries and set members

Set members are ordered by their encodings, and map entries by the
encodings of their keys, then of their values, compared as byte strings
(lexicographically, a proper prefix first). Duplicates are kept.

The requirement is that the same values give the same digest whatever
container holds them: a `HashMap` under any seed, a `BTreeMap`, an
`IndexMap` in any insertion order, a JSON object. Any total order on values
defined by the spec meets it, and the order never appears in a digest, so
the cheapest order to implement correctly is the right one. Comparing
encodings is a `memcmp` in every language; it is sound because encodings
are injective, so equal bytes means equal values.

Natural value order (numbers by value, strings by code point, kinds by a
fixed rank) was the first choice and was dropped after the prior-art review
(D16): it needs a comparator across kinds, numbers and nested values in
every implementation, where a subtle mistake gives silently wrong digests.
No canonical form reviewed sorts by value: CBOR and Preserves sort by
encoded bytes (Preserves does so even though its data model defines a value
order), Ion Hash and objecthash by per-entry hashes, and `stable-hash`
avoids sorting with an additive hash. Sorting by per-entry hashes costs a
SHA-256 per entry; an additive hash is unusual and harder to specify.

Consequences:

- Sorting needs each entry encoded into a buffer (`alloc`), as before; only
  the comparison is cheaper.
- A `BTreeMap` cannot stream in its own order, because Rust's `Ord` almost
  never matches encoded-byte order (a negative integer's encoding starts
  with `0xFF`). Every map and set is sorted.
- The derive sorts a struct's field entries at expansion time, by the
  encodings of their keys, which it can compute. For names without U+0000
  this is code-point order. `Encoder::fields()` checks the order in debug
  builds.

## D14. Encoding framing and numbers

**Everything at the end.** Each value's encoding ends with its own
description: payload, then its length or count if variable, then its kind
tag.

| Kind | Encoding |
|---|---|
| fixed-size values | payload ‖ tag |
| string, bytes (including streams), integer | content ‖ length ‖ tag |
| list, map | items ‖ count ‖ tag |

The encoding is unambiguous because it can be read backwards from the end:
the tag says what kind of value just ended, the length or count says how far
back it reaches. For that, trailing lengths and counts are fixed-width
(8 bytes, big-endian); LEB128 cannot be read backwards reliably. A length
prefix would only serve a decoder reading forwards, and nothing decodes a
digest's input. With lengths at the end, streams and `Display` output are
hashed as they arrive, structs need no pre-pass to count their fields, and
serde sequences of unknown length need no buffer. SHA-256's length
extension is irrelevant: it matters only when a secret is hashed in, and
there are none (D4).

A stream is an API, not a kind: `Encoder::bytes_stream()` (or a string
equivalent) returns a writer, and finishing it writes the length and tag. A
file streamed in has the same digest as the same bytes in memory.

**Integers** have arbitrary precision, encoded as the minimal two's
complement big-endian bytes of ASN.1 DER's INTEGER (X.690 §8.3), followed by
the length and tag. `i128` and `u128` are native; a big-integer feature
could be added without a format change.

**Floats** exactly equal to an integer, at any magnitude, are that integer
(`1e300` included; `-0.0` is the integer 0). Other floats are IEEE 754
binary64, big-endian; `f32` is widened exactly. +∞ is `0x7FF0000000000000`,
−∞ `0xFFF0000000000000`, and every NaN `0x7FF8000000000000`.

## D15. Scope of QUIDDITY_SPEC

`QUIDDITY_SPEC.md` (named to stand apart from other projects' `SPEC.md`
files) uses the conventions of formal specifications: English with RFC 2119
keywords, precise notation for byte layouts, normative test vectors in a
machine-readable file. No machine-checked proofs. Four layers:

1. the data model;
2. the encoding;
3. digests, domains and shortening;
4. the Rust mapping: std and feature types, the derive and its attributes.

Layers 1 to 3 are normative for any implementation. Layer 4 binds only the
Rust crate, and is kept separate.

## D16. Prior-art review

Reviewed before implementation: Ion Hash, CBOR deterministic encoding
(RFC 8949 section 4.2) and dCBOR, Preserves, JCS (RFC 8785), objecthash and
`stable-hash`.

Confirmed:

- **Integral floats as integers** (D14). dCBOR requires the same reduction,
  and a single NaN. Ion and Preserves keep floats and integers distinct;
  JCS makes every number a double.
- **Lengths, not escaping** (D14). Ion Hash delimits values with begin and
  end bytes and escapes them inside values; its list formula omits the
  escaping, leaving the framing of nested lists ambiguous. Postfix lengths
  need no escaping.
- **No Unicode normalization.** Ion and JCS do not normalize; objecthash
  offers NFC only as a separate, optional step.
- **Deferring a pluggable hash** (W6). Ion Hash leaves the hash function to
  the caller and records it nowhere; the `sha256:` text prefix leaves room
  to add algorithms later.
- **Top-level domains** (D4). Ion hashes annotations wherever they occur,
  which is the nested behaviour D4 rejected.

Changed:

- **Canonical order**: by encoded bytes, not by value (D13).
- **`arbitrary_precision` numbers**: digest as without the feature (see the
  drafting choices below).

Recorded for later:

- **Exact decimals** (R4): Ion's decimal kind, where precision is
  significant (`1.0` and `1.00` differ), is the model.
- **Redaction** (R5), from objecthash.

## To revisit

### R4. Richer kinds

The data model is JSON-shaped plus bytes and sets, because comparing Rust
values with JSON is the main use case, and each extra kind (timestamp,
decimal, UUID) breaks equivalence with JSON for its values, which JSON
carries as strings. Proposed: keep the core model as is, but make it
extensible in the way CBOR's tags are, so that typed kinds can be added in
later versions without disturbing existing digests.

The first candidate is an exact decimal (for `rust_decimal`, and for
serde_json numbers that binary64 rounds), modelled on Ion's decimal, where
precision is significant. Tag `0x12` is free for it.

### R5. Redaction (potential future addition)

Not needed now. objecthash lets part of a value be replaced by its hash
while the overall digest stays the same, so that one field can be shown
without revealing the others. Applied to every node, this needs a hash per
value and a different encoding; quiddity's flat encoding does not offer it.

An opt-in form is a small, purely additive extension:

- A **digest kind** (e.g. tag `0x22`) whose content is the 32-byte
  structural digest of a value.
- A field attribute, `redactable`, under which the field is always encoded
  as the digest of its value, never as the value itself.
- An `Encoder` method that writes a digest already in hand, so that a
  verifier given the redacted form can rebuild the parent's encoding and
  check the top-level digest.

Fields without the attribute cost nothing. Marking a field `redactable`
changes the type's digests, so it is chosen when the type is designed, like
`domain`. A struct with a redactable field no longer has the digest of the
plain JSON object, unless the JSON side marks that value the same way.

The digest of a guessable value (a boolean, an age) reveals it: the value
can be found by hashing each candidate. objecthash adds a random 32-byte
salt to each value for this; with quiddity, a value to be hidden would need
to be stored with a salt, by the caller.


## Desiderata

Wanted, but not yet committed to. If the trade-offs turn out badly, they may
be dropped or changed.

### W2. No space for a domain in a `Digest` that has none

Met by D4: a domain is hashed in, not carried, so no `Digest` holds one.

### W4. Reading labels from a digest

Reading a digest's domain (or the type it came from) at run time is not
possible under D4: a digest is hash bytes only. Within a program, a type
parameter (`Digest<I>`, or a domain marker) could label digests at no run-time
cost; for stored digests, the text form could carry labels
(`example.task:sha256:…`). Not needed for comparing a struct with JSON (D4).
Undecided.

### W7. Identities for dynamic data

An identity as runtime data (field inclusion and exclusion, renames,
omission rules, nested identities) that can be applied to a dynamic value
such as a `serde_json::Value`, giving exactly the digest the derive gives the
equivalent struct under that identity. Written as data (e.g. JSON), such an
identity could be shared between languages, which a Rust derive cannot.
It depends only on spec layers 1 to 3 (D15), so it can be designed after the
spec, as a module or a separate crate. A separate issue.

### W6. Pluggable hash function (deferred)

Making the hash function a parameter (e.g. generic over the `digest`
crate's trait) is easy to build, but every supported algorithm becomes part
of the format: digests of different algorithms cannot be compared or
converted, the text form must name the algorithm, the domain step must use
the same one, other-language implementations must support each, and a
non-cryptographic choice (xxh3) gives up collision resistance. Speed gains
are mostly for large values; for JSON-sized data the cost is dominated by
encoding. Deferred: SHA-256 only, until there is a need.

## Notes for SPEC.md

Details for the encoding specification, collected here until `SPEC.md` is
written. Each needs a precise statement there.

Prior art: reviewed in D16.

Details:

- Strings formatted from `Display` (`encode_as = display`) are encoded
  exactly as other strings (D6).
- An identity is not part of a digest. It determines which fields are
  encoded and how, and affects the digest only through them (D4). Two
  identities that encode a value the same way give it the same digest; a
  `Task` digest under `Meaning` equals the digest of a JSON object with the
  same fields and values. Renaming a marker type does not change digests.
- A nested value is digested under its own primary identity unless its
  field has `identity = I` (D3).
- Domain framing: distinct tags for the structural and in-domain hash
  inputs, and a length-prefixed label (D4).
- Domains apply to the top-level digest only (D4).
- The data model and the mapping of Rust forms onto it (Goal section), with
  the default shapes and `encode_as` rules (D6).
- Framing (D14): each value ends with its tag, preceded by its fixed-width
  length or count when variable; the tag values are chosen in the spec.
- Canonical order of map entries and set members (D13).
- Numbers (D14): DER integers, exactly-integral floats as integers,
  binary64 otherwise, canonical infinities and NaN.
- Leaf mappings stated in the README's "Implementations on std types":
  `Vec<u8>` and `[u8; N]` are lists of numbers, as in serde's JSON (bytes
  only via `with` and `Encoder::bytes`); `bool` is not a number; `char` is a
  one-character string; `()` is null; `Box`/`Rc`/`Arc`/`&` are transparent;
  `f32` is widened to `f64` by value, so `0.1f32` and `0.1f64` differ.
- Schema changes and their effect on digests, for a struct encoded as a map
  and an enum encoded as strings (the defaults). Removed from the README in
  favour of the spec; the list is examples, not a complete statement:

  | Change | Digest |
  |---|---|
  | Reorder fields or enum variants | unchanged |
  | Rename a field or variant, with `rename = "<old name>"` | unchanged |
  | Rename or move the type | unchanged |
  | Add an `Option` field with `none_as_missing` whose value is `None` | unchanged |
  | Add a field with `omit_if` whose value passes the check | unchanged |
  | Change `T` to `Option<T>` (the value being `Some`) | unchanged |
  | Widen an integer (`u32` → `u64`, `i32` → `i128`) | unchanged |
  | Change `f32` → `f64`, for values `f32` represents exactly | unchanged |
  | Change an integer to a float, with an integral value | unchanged |
  | Switch between `Vec<T>`, `[T; N]`, `VecDeque<T>` and slices | unchanged |
  | Switch between `HashMap`, `BTreeMap` and `IndexMap` | unchanged |
  | Switch between `String`, `&str`, `Box<str>` and `Cow<str>` | unchanged |
  | Wrap in `Box`, `Rc`, `Arc` or `&` | unchanged |
  | Add a field with any other value | changed |
  | Change a field's value | changed |
  | Change a list into a set (`Vec<T>` → `BTreeSet<T>`) | changed |
  | Add, remove or change the `domain` | changed |
  
  For a struct with `encode_as = list`, renaming a field leaves its digests
  unchanged, and any other change to its fields changes them. For an enum
  with `encode_as = int`, renaming a variant leaves its digests unchanged, and
  reordering variants changes them unless their discriminants are written
  explicitly.

## Choices made while drafting QUIDDITY_SPEC

Decided in the draft, open to review:

- Tags: null `0x01`, false `0x02`, true `0x03`, integer `0x10`, float
  `0x11`, string `0x20`, bytes `0x21`, list `0x30`, set `0x31`, map `0x32`.
- Hash input prefixes: `quiddity/1/value` (structural) and
  `quiddity/1/domain`; the domain input is prefix ‖ label ‖ u64be(label
  length) ‖ structural digest. Domain labels must be non-empty.
- Text forms: lowercase prefixes and hex on output; uppercase hex accepted
  on input.
- A `Digest` used as a value is the string of its text form (equal to its
  serde JSON form).
- `PhantomData<T>` is null as a value. As a field of a derived struct or
  variant it is absent, as if skipped (decided after the draft: a phantom
  field carries no data, so it should not appear in the encoding; null was
  the fallback had this been awkward to implement). Implementation: a hidden
  associated const on `Quiddity` (`const ABSENT_AS_FIELD: bool = false`,
  `true` for `PhantomData`) that the derive tests, rather than matching the
  type's name, so that type aliases and generic parameters instantiated with
  `PhantomData` behave the same. The `transparent` field count then needs a
  post-monomorphization const assertion for generic field types.
- `none_as_missing` and `omit_if` on a list-encoded struct are compile
  errors. A scoped `domain` is allowed (only `identities` cannot be
  scoped).
- Strings with surrogate code points are rejected, not repaired.
- serde_json `arbitrary_precision` numbers digest as they would without the
  feature: an integer within `i64` or `u64` is that integer, and anything
  else is rounded to binary64 and normalized. Like `preserve_order`, the
  feature can be enabled by any crate in the dependency graph, so a digest
  must not depend on it. JCS and I-JSON also round to binary64. serde_json
  itself keeps the text under the feature; exact decimals belong to R4.
- The serde serializer reports `is_human_readable() == true`, so types such
  as `Uuid` and timestamps serialize as their string forms, matching JSON.
- The worked examples in the spec's Appendix A, and the normative vectors
  in `test-vectors/quiddity-v1.json`, come from the Python implementation
  in `test-vectors/qref.py` (`generate.py --check` detects a stale vectors
  file). The
  vectors use a typed JSON notation, since plain JSON cannot express bytes,
  sets, non-string keys or a float distinct from an integer. If the vectors
  and the spec's prose disagree, the prose wins.

## Open items

- `QUIDDITY_SPEC.md`: drafted (version 1, draft) and read through. Test
  vectors are in `test-vectors/`. The directory is named for the
  conventional term (BLAKE3, CBOR, Wycheproof); `conformance` usually names a
  test harness, which this is not.
- R4: richer kinds (exact decimals first).
- R5: redaction (potential future addition).
- W6: pluggable hash function (deferred).
- W7: identities for dynamic data (separate issue).
- W4: labels readable from digests (typed `Digest<I>`, text form).
- License: MIT for now.
