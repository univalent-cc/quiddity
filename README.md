# quiddity

*Quiddity* (n.): the essence of a thing; what makes it *what it is*, as
distinct from its incidental properties.

`quiddity` computes stable digests of values from the fields you mark as
essential using derive macro attributes.

Properties of `quiddity` include:

- **stability** across processes, platforms, Rust versions and releases of
  this crate.  Depending on the configuration, it may also be robust against the
  underlying containers, type layouts, in-memory representation, etc.  The 
  encoding is documented in [`QUIDDITY_SPEC.md`], and a semver-compatible release
  of `quiddity` will never change the digest of a value.
- **control over fields**. Limit the hash to specified fields, so incidental 
  fields like timestamps, hints, caches, etc. don't affect the digest.
- **support for multiple identities**. The same struct may be hashed differently
  depending on the purpose of the digest.  Different digest identities may allow
  for different field subsets, field encoding parameters, etc.

```toml
[dependencies]
quiddity = { version = "0.1", features = ["serde_json"] }
```

## Example

```rust
use quiddity::{Digest, Quiddity, QuiddityExt};

#[derive(Quiddity)]
pub struct Task {
    prompt: String,
    documents: Vec<DocumentRef>,
    priority: u8,

    /// Not part of the digest.
    #[quiddity(skip)]
    submitted_at: std::time::SystemTime,
}

#[derive(Quiddity)]
pub struct DocumentRef {
    label: String,
    content: Digest, // a digest is itself hashable
}

let task = Task {
    prompt: "Summarize the report".into(),
    documents: vec![DocumentRef {
        label: "report".into(),
        content: "The report's text".quiddity(),
    }],
    priority: 1,
    submitted_at: std::time::SystemTime::now(),
};

let digest: Digest = task.quiddity(); // every field except `submitted_at`

println!("{digest}"); // sha256:9f2c…
```

A [`Digest`] is written as text in the form `sha256:<64 hex digits>`.
The `Display` and `Serialize` implementations write this format and `FromStr` parses it.

This example uses a digest as an HTTP `ETag` to detect concurrent changes:

```rust ignore
let current = task.quiddity();
let claimed: Digest = request.header("If-Match")?.parse()?;

if claimed != current {
    return Err(StatusCode::PRECONDITION_FAILED);
}
```

## Additional Identities

The fields and parameters that inform the digest logic are an *identity*. Every
type has a [`Primary`] identity, and a type can also declare additional identities,
corresponding to types implementing the [`IdentityMarker`] trait.

```rust
use quiddity::{Digest, IdentityMarker, Quiddity, QuiddityExt};

// Declare an IdentityMarker type to define the `Meaning` identity
#[derive(IdentityMarker)]
pub struct Meaning;

#[derive(Quiddity)]
#[quiddity(identities(Meaning))]
pub struct Task {
    prompt: String,

    // Left out of `Meaning`
    #[quiddity(skip(Meaning))]
    priority: u8,

    // Part of `Meaning`, but not the `Primary` identity
    #[quiddity(skip(Primary))]
    answer_format: String,

    /// Not part of any identity.
    #[quiddity(skip)]
    submitted_at: std::time::SystemTime,
}

let task = Task {
    prompt: "Summarize the report".into(),
    priority: 1,
    answer_format: "one paragraph".into(),
    submitted_at: std::time::SystemTime::now(),
};

let primary: Digest = task.quiddity();               // the prompt and priority
let meaning: Digest = task.quiddity_as::<Meaning>(); // the prompt and answer format
```

An attribute written without an identity applies to every identity. An
attribute written with an identity in parentheses applies to that identity
only, and overrides the unscoped form for it. So `skip` leaves a field out
of every identity, `skip(Meaning)` leaves it out of `Meaning` only, and
`skip(Primary)` leaves it out of the primary identity only. See [Scoping
attributes to an identity](#scoping-attributes-to-an-identity).

A type with identity `I` implements `Quiddity<I>`, so an identity marker type
can be a generic parameter.

## Domains

Sometimes we want to encode type or provenance information into a digest.  By default,
a digest only depends on the contents of a container, so two types with the same
field names and values would have the same digest.

This could be a problem when digests are signed. For example, if a signature
over the digest of a `Payment` is accepted as authorizing that payment, the same
signature must not also verify the `Refund`, even though both types contain exactly
the same fields. The `domain` attribute makes the digests also depend on the
domain name:

```rust
use quiddity::{Quiddity, QuiddityExt};

#[derive(Quiddity)]
#[quiddity(domain = "example.payment")]
struct Payment { to: String, amount: u64 }

#[derive(Quiddity)]
#[quiddity(domain = "example.refund")]
struct Refund { to: String, amount: u64 }

let payment = Payment { to: "bob".into(), amount: 100 };
let refund = Refund { to: "bob".into(), amount: 100 };
assert_ne!(payment.quiddity(), refund.quiddity());
```

A type has no domain unless one is explicitly set. A type with a domain has its
digest computed in two steps: the structural digest of the contents is computed
first, then the domain label is hashed into the digest. Types may share a domain,
for example an owned type and a borrowed view. The domain is  applied to the
top-level value only; nested values are digested structurally.

The [`in_domain`] method lifts a structure-only digest into
a domain. This allows untyped data to be compared with a digest in a domain:

```rust ignore
let from_json = json.quiddity().in_domain("example.payment");
assert_eq!(from_json, payment.quiddity());
```

IMPORTANT: Domains do not make digests unforgeable: the encoding is public,
and anyone can compute any digest in any domain. A domain allows a digest to
encode provenance, so that a secure signature cannot be reused for a value
of another kind.

## Attributes

### On a type

| Attribute | Meaning |
|---|---|
| `domain = "…"` | A label hashed into every digest of this type, so that its digests differ from those of other types with the same fields (see [Domains](#domains)). None by default. |
| `identities(A, B, …)` | The named identities this type implements, in addition to the primary one. Each is a type deriving `IdentityMarker`. |
| `transparent` | The type digests as its single field. Exactly one field must remain, after skips, for every identity it applies to, or it is a compile error. |
| `encode_as = …` | `map` or `list` for a struct with named fields (default = `map`), `string` or `int` for an enum (default = `string`). `display` for any type that implements `Display`, encode the output. `serde` (with the `serde` feature) for any type that implements `Serialize`, encode it through serde; panics if serialization fails (see [Direct from `serde`](#direct-from-serde)). |

### On a field

| Attribute | Meaning |
|---|---|
| `skip` | Exclude the field from the digest. |
| `rename = "…"` | Provide another name that the field or enum variant is hashed under. |
| `identity = A` | Encode this nested value using its identity `A` instead of its primary one. |
| `with = path` | Digest this field with `path(&value, &mut Encoder)`, for types that do not implement `Quiddity` (see [Foreign types](#foreign-types-and-custom-field-encoding)). |
| `encode_as = display` | Encode this field as its `Display` output, a string. For types that implement `Display` but not `Quiddity`. |
| `encode_as = serde` | With the `serde` feature: encode this field through its `Serialize` impl. Panics if serialization fails (see [Direct from `serde`](#direct-from-serde)). |
| `none_as_missing` | When the field is `None`, hash it as a missing key rather than as null. Only valid on `Option` fields. |
| `omit_if = path` | Omit the field when `path(&value)` returns `true` (e.g. `omit_if = Vec::is_empty`), so that adding the field does not change existing digests. The cost of the check is the caller's. |

### On an enum variant

`rename = "…"` works as it does for fields.  Other attributes do not apply.

### Scoping attributes to an identity

Every attribute except `identities(…)` can be limited to one or more
identities by naming them in parentheses after the attribute's name:

```rust ignore
#[quiddity(skip(Meaning))]                 // skipped under `Meaning` only
#[quiddity(skip(Primary))]                 // skipped under the primary identity only
#[quiddity(rename(Meaning) = "question")]  // a different name under `Meaning`
#[quiddity(skip(Meaning, Routing))]        // skipped under both
#[quiddity(skip, skip(Meaning) = false)]   // skipped under every identity except `Meaning`
```

`Primary` does not need to be imported to use it as a macro arg.

## Shorter digests

[`Digest::mini()`] returns the first 128 bits of a digest, as a [`Digest128`],
and [`Digest::micro()`] the first 64 bits, as a [`Digest64`]:

```rust
use quiddity::{Digest64, QuiddityExt};

let etag: Digest64 = "Summarize the report".quiddity().micro();
assert_eq!(etag.to_string(), "sha256-64:366d2ab7f2c3e7db");
```

They save space where many digests are stored or sent; computing costs
the same as a full digest so this is a usage-time optimization / tradeoff.

64 bits is enough to compare two known values. It is not enough for large
collections (collisions become likely around 2³² digests), or where digests
must resist deliberate collisions, as with secure signatures.

## Cargo features

| Feature | Default | Provides | Requires |
|---|---|---|---|
| `derive` | yes | `#[derive(Quiddity)]` and `#[derive(IdentityMarker)]` | |
| `std` | yes | `HashMap` and `HashSet` | `alloc` |
| `alloc` | yes | `String`, `Vec`, `VecDeque`, `Box`, `Rc`, `Arc`, `Cow`, `BTreeMap` and `BTreeSet` | |
| `serde_json` | no | `Value`, `Map` and `Number` | `alloc` |
| `serde` | no | `encode_as = serde` and `quiddity::serde::digest`; `Serialize` and `Deserialize` for `Digest`, `Digest128` and `Digest64` | `alloc` |
| `indexmap` | no | `IndexMap` and `IndexSet` | `alloc` |
| `uuid` | no | `Uuid` | |
| `bytes` | no | `Bytes` and `BytesMut` | `alloc` |
| `url` | no | `Url` | `std` |

With `default-features = false`, `quiddity` is `no_std` and needs no
allocator. The derive (with the `derive` feature), `Digest` and its shorter forms, numbers, `bool`,
`char`, `&str`, slices, arrays, tuples, `Option`, references and
`encode_as = display` all work without `std` or `alloc`:

```toml
[dependencies]
quiddity = { version = "0.1", default-features = false, features = ["derive"] }
```

## Implementations of the Quiddity trait

- `String`, `&str`, `Box<str>` and `Cow<str>` are all strings, with the same
  digest for the same text.
- `None` is digested as null, and `Some(x)` as `x`. In a struct, a `None`
  field is null unless it has `none_as_missing`.
- `Box`, `Rc`, `Arc` and references are digested as the value they point to.
- `char` is digested as a one-character string.
- `bool` is digested as `true` or `false`.
- `()` is digested as null.

### Number Primitives

Numbers are digested by value, not by type. `5u8`, `5i64`, `5u128` and
`5.0f64` all have the same digest. So do `-0.0` and `0`, and every NaN.

Widening a field from `f32` to `f64` keeps its digest only for values that
`f32` represents exactly: `0.1f32` is not the number `0.1f64`, so their
digests differ.

`bool` is not a number: `true` and `1` have different digests.

### HashMap, BTreeMap, etc.

A map is digested by its entries, not by the order it iterates in. A
`HashMap`, a `BTreeMap` and an `IndexMap` with the same entries have the
same digest. A map with string keys also has the same digest as a struct
with those field names, and as a JSON object with those keys:

```rust
use std::collections::{BTreeMap, HashMap};
use quiddity::{Quiddity, QuiddityExt};

#[derive(Quiddity)]
struct Interval { start: u64, end: u64 }

let btree_map = BTreeMap::from([("start", 10u64), ("end", 15)]);
let hash_map = HashMap::from([("end", 15u64), ("start", 10)]);
let interval = Interval { start: 10, end: 15 };

assert_eq!(btree_map.quiddity(), hash_map.quiddity());
assert_eq!(btree_map.quiddity(), interval.quiddity());
```

Sets (`HashSet`, `BTreeSet`) are likewise unordered, but a set is not a
list: `BTreeSet::from([1, 2])` and `vec![1, 2]` have different digests.

### Tuples, arrays and Vec

`Vec<T>`, `VecDeque<T>`, arrays, slices and tuples are all lists, digested
in order. `vec![1, 2]`, `[1u8, 2]` and `(1u32, 2i64)` have the same digest as
the JSON array `[1, 2]`. Reordering the items changes the digest.

`Vec<u8>` and `[u8; N]` are lists of numbers too, as they are in serde's
JSON. To digest a field as raw bytes instead, use `with` and
[`Encoder::bytes`], as in the UUID example below.

### `serde_json`

The `serde_json` feature implements `Quiddity` for `Value`, `Map` and
`Number`, following JSON's semantics rather than serde_json's in-memory
representation:

- Object keys are unordered, as RFC 8259 defines them: `{"a":1,"b":2}` and
  `{"b":2,"a":1}` have the same digest. This does not depend on serde_json's
  `preserve_order` feature, which Cargo may enable through any crate in the
  dependency graph.
- An object has the same digest as a struct with the same field names and
  values. JSON `null` hashes as a Rust `None`, and a missing
  key as a `None` field with `none_as_missing`.
- Numbers are compared by value: `1`, `1.0` and `1e0` have the same digest,
  which is also the digest of an integer field holding `1`.
- serde_json's `arbitrary_precision` feature does not change digests either:
  a number digests as it would without the feature. A number that is
  neither an integer within `i64` or `u64` nor exactly representable as an
  `f64` is rounded to the nearest `f64`.
  serde_json's default parser can itself be off by one in the last bit of a
  float; its `float_roundtrip` feature makes parsing exact.
- Arrays are ordered.
- Strings are not Unicode-normalized, here or anywhere else: `é` as one code
  point and as `e` plus a combining accent are different strings, with
  different digests. Normalize text before hashing if it may contain both.

### `indexmap`, `uuid`, `bytes` and `url`

Each of these features implements `Quiddity` for the named crate's types:

- `indexmap`: `IndexMap` and `IndexSet` are digested as maps and sets, like
  their `std` counterparts. Insertion order does not affect the digest.
- `uuid`: a `Uuid` is digested as its hyphenated lowercase string
  (`"67e55044-10b1-426f-9247-bb680e5fe0c8"`), the form serde writes in JSON,
  so it has the same digest as that JSON string. To digest it as 16 bytes
  instead, see below.
- `bytes`: `Bytes` and `BytesMut` (from the `bytes` crate) are digested as
  bytes. Unlike a `Vec<u8>`, they do not have the same digest as a JSON
  array of numbers.
- `url`: a `Url` is digested as its string form.

### Direct from `serde`

The `serde` feature digests any value through its `serde::Serialize` impl.
It covers types from crates that `quiddity` has no feature for, such as
`chrono` or `bson`:

```rust ignore
#[derive(Quiddity)]
struct Event {
    #[quiddity(encode_as = serde)]
    at: chrono::DateTime<chrono::Utc>,
}

let digest = quiddity::serde::digest(&config)?; // any `Serialize` value
```

Values are digested in the shapes serde gives them: a struct as a map, a
sequence as a list, a unit variant as its name, and so on. So a `Serialize`
value has the same digest as the equivalent `Quiddity` value. This feature
is separate from `serde_json`, and does not go through `serde_json::Value`:
bytes stay bytes, and integers wider than 64 bits are kept.

It is slower than a native implementation, and may allocate. If a
`Serialize` impl returns an error, [`quiddity::serde::digest`] can propagate
the error;  However, the infallible `quiddity` method will panic if a `serde`
error is encountered.

### Foreign types and custom field encoding

The orphan rule prevents implementing `Quiddity` for a type defined in
another crate. A type that implements `Display` can be digested as its
`Display` output with `encode_as = display`:

```rust
use quiddity::Quiddity;

#[derive(Quiddity)]
struct Host {
    #[quiddity(encode_as = display)]
    address: std::net::IpAddr,
}
```

With the `serde` feature, `encode_as = serde` digests a field through its
`Serialize` impl (see above). Otherwise, the `with` attribute
digests a field with a function of your own. It also
overrides a type's own encoding, for example to digest a `Uuid` as its 16
bytes rather than as a string:

```rust
use quiddity::Quiddity;

#[derive(Quiddity)]
struct Event {
    #[quiddity(with = digest_uuid)]
    id: uuid::Uuid,
}

fn digest_uuid(id: &uuid::Uuid, e: &mut quiddity::Encoder) {
    e.bytes(id.as_bytes());
}
```

[`Encoder`] writes the primitive kinds (`null`, `bool`, `int`, `float`, `str`,
`bytes`) and the composite ones (`list`, `set`, `map`, `fields`). A manual
`Quiddity` impl uses the same API.

### Manually implementing the Quiddity trait

When the derive does not fit, it is possible to implement [`Quiddity`] directly.
`encode` writes the value through the `Encoder`. This type is stored as a start
and a length, but digested as a start and an end, the form other systems send it
in:

```rust
use quiddity::{Encoder, Quiddity};

pub struct Interval {
    start: u64,
    len: u64,
}

impl Quiddity for Interval {
    const DOMAIN: Option<&'static str> = None; // the default; may be omitted

    fn encode(&self, e: &mut Encoder) {
        let mut f = e.fields();
        // Fields must be written in sorted order (checked in debug builds).
        f.field("end", &(self.start + self.len));
        f.field("start", &self.start);
        f.finish();
    }
}
```

`Interval { start: 10, len: 5 }` has the same digest as the JSON object
`{"start": 10, "end": 15}`.

`impl Quiddity for T` implements the primary identity. To implement others,
implement `Quiddity<I>` for them. A container can implement every identity,
encoding its items under whichever identity it is encoded under:

```rust
use quiddity::{Encoder, IdentityMarker, Quiddity, QuiddityExt};

/// A ring buffer: `items[head]` is its first item.
pub struct Ring<T> {
    items: Vec<T>,
    head: usize,
}

impl<T> Ring<T> {
    fn iter_from_head(&self) -> impl Iterator<Item = &T> {
        self.items[self.head..].iter().chain(&self.items[..self.head])
    }
}

impl<I: IdentityMarker, T: Quiddity<I>> Quiddity<I> for Ring<T> {
    fn encode(&self, e: &mut Encoder) {
        let mut l = e.list();
        for item in self.iter_from_head() {
            l.item_as::<I, _>(item);
        }
        l.finish();
    }
}

let ring = Ring { items: vec![3, 1, 2], head: 1 };
assert_eq!(ring.quiddity(), vec![1, 2, 3].quiddity());
```

### Requesting other implementations

Support for more crates and formats is welcome, for example `smallvec`,
`arrayvec`, `chrono`, `time`, `rust_decimal`, `bson`, Amazon Ion, CBOR,
MessagePack, TOML or YAML values. Please open an issue and / or a pull request,
if you would like to see an implementation for another type.  Please say how
the type's values should be digested. In the meantime, the `serde` feature
covers most types that implement `Serialize`.

## Alternatives to Quiddity

This section lists crates that address parts of the same problem, what each
provides, and where each differs from `quiddity`.

The problem has three parts:

1. **Selection**: declaring which fields matter for the digest.
2. **Stability**: a specified encoding that does not change.
3. **Equivalence**: comparability between different container types, permits an evolving schema.

### Derives that skip fields from `std::hash::Hash`

[`derive-where`](https://crates.io/crates/derive-where),
[`educe`](https://crates.io/crates/educe) and
[`derivative`](https://crates.io/crates/derivative) derive `Hash` (among
other traits) with fields skipped. They provide selection, for uses such as
`HashMap` keys and in-memory deduplication.

They do not provide stable digests, because `std::hash::Hash` does not: how
a type feeds a `Hasher` is unspecified, and has varied by platform (`usize`
is written at native width) and by Rust version (how strings are
terminated). [`rustc-stable-hash`](https://crates.io/crates/rustc-stable-hash)
publishes rustc's stable hasher, which fixes the hasher but not what `Hash`
feeds into it. rustc's `HashStable` derive, which has an `ignore` attribute,
is internal to the compiler. `derivative`'s last release was in 2021;
`derive-where` and `educe` are maintained.

### Canonical serialization formats

A canonical format encodes each value as exactly one byte string, and a hash
of that string is a stable digest. These formats provide stability.
Selection comes from the format's skip attribute, which also removes the
field from the format, and allows one choice of fields per type.

- [`borsh`](https://crates.io/crates/borsh) (NEAR; "Binary Object
  Representation Serializer for Hashing"). A binary format with a written
  specification, its own derive, and `#[borsh(skip)]`. It is maintained,
  widely used (including by Solana), sorts hash maps before encoding, and
  rejects NaN. Its derive is separate from serde's, so skipping
  a field from borsh does not affect JSON serialization. It supports one
  choice of fields per type. Its encoding is positional: adding, removing or
  reordering a field changes every digest, as does `u32` → `u64`. It has no `serde_json::Value` impl; one was proposed
  and declined in favour of an example (near/borsh-rs#312, #317), so `Value`
  fields need `#[borsh(serialize_with = …)]`. That example encodes object
  keys in map order.
- [`bcs`](https://crates.io/crates/bcs) (Binary Canonical Serialization,
  from Diem; used by Aptos and Sui). A canonical binary format driven by
  serde, so a field is skipped with `#[serde(skip)]`, which also removes it
  from JSON serialization. Aptos has derives in `aptos-core` (`CryptoHasher`,
  `BCSCryptoHash`) that prefix a per-type salt for domain separation.
  Positional, like borsh.
- [`serde_ipld_dagcbor`](https://crates.io/crates/serde_ipld_dagcbor)
  (IPLD). DAG-CBOR, the canonical CBOR used for content addressing in IPFS.
  serde-driven, with the same skip behaviour. Its content identifiers (CIDs)
  interoperate with the IPLD ecosystem.
- [`tree_hash`](https://crates.io/crates/tree_hash) and
  [`ethereum_ssz`](https://crates.io/crates/ethereum_ssz) (Sigma Prime). SSZ
  and its Merkle hash, as used by Ethereum consensus. A Merkle root allows
  proving one field without revealing the others. Designed for Ethereum's
  fixed schemas.

### JSON canonicalization (RFC 8785)

[`serde_jcs`](https://crates.io/crates/serde_jcs) and
[`serde_json_canonicalizer`](https://crates.io/crates/serde_json_canonicalizer)
implement the JSON Canonicalization Scheme: sorted object keys, ECMAScript
number formatting, and fixed string escaping. Each encodes a serde value as
exactly one JSON string, which provides stability. Because the canonical
form is JSON, any language with a JCS implementation can recompute the
digest.

JCS specifies an encoding, not which fields are encoded. With serde,
selecting fields means `#[serde(skip)]`, which also changes the type's JSON
serialization. The common pattern is a second, borrowing struct holding the
fields that count, serialized with JCS and then hashed. Which fields count is
then recorded in that second struct, one per identity, and kept in step with
the original by hand. Adding a field changes every digest unless the field is
skipped when empty, and renaming a field always does.

### Schema-tolerant hashing

- [`stable-hash`](https://crates.io/crates/stable-hash) (The Graph). Hashes
  are addressed by each field's position in the structure, a field at its
  default value contributes nothing (so adding an optional field leaves old
  hashes unchanged), and integers can widen. It provides stability and
  evolution. Its `impl_stable_hash!` macro destructures the struct, so every
  field must be listed: it has no skipping and no named identities. Its last
  release was in 2022.
- **objecthash** (Ben Laurie). Hashes each field and combines the hashes, so
  that a digest can be checked with some fields redacted, and computed the
  same way from JSON, protobuf or other encodings. The Rust ports,
  [`objecthash`](https://crates.io/crates/objecthash) and
  [`blot`](https://crates.io/crates/blot), have had no releases since 2017
  and 2018.

### `digestible`

[`digestible`](https://crates.io/crates/digestible) has an API similar to
this crate's: a derive, `#[digestible(skip)]`, `digest_with` for foreign
types, optional type headers for domain separation, any
[`digest`](https://crates.io/crates/digest) hash function, and a
`serde_json::Value` impl. As of 0.3.0, its digests are not stable:

- `HashMap` and `HashSet` are digested in iteration order, which Rust
  randomizes per process.
- Strings are not length-prefixed, so `("ke", "yvalue")` and
  `("key", "value")` produce the same digest.
- Enum variants and JSON kinds are not tagged: JSON `null` and `false` both
  digest as a single `0` byte.
- Byte order is a type parameter, and `digest_native` uses the machine's.

These are properties of its encoding, so fixing them would change every
digest the crate produces.

### Summary

| Crate | Selection on the type | Stable encoding | Survives evolution | Several identities | `serde_json::Value` |
|---|---|---|---|---|---|
| `derive-where`, `educe` | yes | no | — | no | — |
| `borsh` | yes (`skip`) | yes, specified | no (positional) | no | no (example only) |
| `bcs`, DAG-CBOR | via serde (also removes it from JSON) | yes | no | no | via serde |
| `serde_jcs`, `serde_json_canonicalizer` | via serde, or a projection struct | yes, RFC 8785 | partly | one struct each | yes |
| `stable-hash` | no | yes | yes | no | no |
| `digestible` | yes (`skip`) | no | no | no | yes |
| `quiddity` | yes | yes, specified | yes | yes | yes, unordered keys |

## Stability

A semver-compatible release does not change the digest of any value. Any
change to the encoding will be done in a semver-incompatible release, and
the changelog will list the values it affects.

## License

Licensed under the MIT license.

[`QUIDDITY_SPEC.md`]: https://github.com/univalent-cc/quiddity/blob/main/QUIDDITY_SPEC.md
[`Quiddity`]: https://docs.rs/quiddity/latest/quiddity/trait.Quiddity.html
[`Digest`]: https://docs.rs/quiddity/latest/quiddity/struct.Digest.html
[`Digest128`]: https://docs.rs/quiddity/latest/quiddity/struct.Digest128.html
[`Digest64`]: https://docs.rs/quiddity/latest/quiddity/struct.Digest64.html
[`Digest::mini()`]: https://docs.rs/quiddity/latest/quiddity/struct.Digest.html#method.mini
[`Digest::micro()`]: https://docs.rs/quiddity/latest/quiddity/struct.Digest.html#method.micro
[`in_domain`]: https://docs.rs/quiddity/latest/quiddity/struct.Digest.html#method.in_domain
[`IdentityMarker`]: https://docs.rs/quiddity/latest/quiddity/trait.IdentityMarker.html
[`Primary`]: https://docs.rs/quiddity/latest/quiddity/struct.Primary.html
[`Encoder`]: https://docs.rs/quiddity/latest/quiddity/struct.Encoder.html
[`Encoder::bytes`]: https://docs.rs/quiddity/latest/quiddity/struct.Encoder.html#method.bytes
[`quiddity::serde::digest`]: https://docs.rs/quiddity/latest/quiddity/serde/fn.digest.html
