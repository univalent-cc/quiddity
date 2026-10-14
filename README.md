# quiddity

*Quiddity* (n.): the essence of a thing; what makes it *what it is*, as
distinct from its incidental properties.

`quiddity` computes stable digests of values from the fields you mark as
essential.

Properties of `quiddity` include:

- **stability** across processes, platforms, languages and releases. The
  encoding is documented in [`QUIDDITY_SPEC.md`](QUIDDITY_SPEC.md), and the
  same value has the same digest in every implementation.
- **control over fields**. Limit the hash to specified fields, so incidental
  fields like timestamps, hints, caches, etc. don't affect the digest.
- **support for multiple identities**. The same struct may be hashed differently
  depending on the purpose of the digest.  Different digest identities may allow
  for different field subsets, field encoding parameters, etc.

Every implementation is tested against the shared vectors in
[`test-vectors/`](test-vectors/).

## Elixir

Coming soon

## Python

Coming soon

## Rust

The Rust crate in [`rust/`](rust/) includes:

- A derive macro to generate encodings at compile time.
- Implementations for most `std` types
- Optional features for types from `serde_json`, `indexmap`, `uuid`, `bytes` and `url`.
- Support for any type that implements serde's `Serialize`.
- Compatibility with `no_std` environments.

## Alternatives to Quiddity

This section lists libraries and formats that address parts of the same
problem, what each provides, and where each differs from `quiddity`.

The problem has three parts:

1. **Selection**: declaring which fields matter for the digest.
2. **Stability**: a specified encoding that does not change.
3. **Equivalence**: comparability between different container types, permits an evolving schema.

### Summary

| Library or format | Languages | Field selection | Stable encoding | Survives evolution | Several identities | JSON values |
|---|---|---|---|---|---|---|
| `derive-where`, `educe` | Rust | on the type | no | — | no | — |
| `digestible` | Rust | on the type | no | no | no | yes |
| `object-hash` | JavaScript | per call | by policy, not specified | no | per call | yes |
| `hashstructure` | Go | on the type | not specified | partly | no | no |
| `DeepHash` (deepdiff) | Python | per call | not documented | no | per call | yes |
| `borsh` | Rust, JavaScript, others | on the type | yes, specified | no (positional) | no | no (example only) |
| BCS, DAG-CBOR | Rust, Go, JavaScript, Python, others | via serde in Rust (also removes it from JSON) | yes | no | no | via serde |
| JCS (RFC 8785) | many | a projection struct | yes, RFC 8785 | partly | one struct each | yes |
| `stable-hash` | Rust | no | yes | yes | no | no |
| objecthash | Python, C, Go, Ruby, JavaScript | no (redaction instead) | yes, specified | no | no | yes |
| Ion Hash | Java, JavaScript, Python, Go, .NET, Rust | no | yes, specified | no | no | via Ion |
| `quiddity` | Rust; Python and Elixir planned | on the type | yes, specified | yes | yes | yes, unordered keys |

### Object hashers in other languages

These hash a language's own values, with options for which fields count and
which containers are unordered. None has a specified encoding, so none can
be recomputed in another language.

- [`object-hash`](https://github.com/puleos/object-hash) (JavaScript).
  Fields are excluded per call, with an `excludeKeys` function. Objects and
  sets are unordered by default, and arrays can be. The default hash is
  SHA-1. Since version 1.1.8, a change to hash values is a semver-major
  release.
- [`hashstructure`](https://github.com/mitchellh/hashstructure) (Go).
  Struct tags exclude a field (`hash:"ignore"`), treat a slice as a set
  (`hash:"set"`), or hash a field's `String()` output (`hash:"string"`). An
  option skips fields holding zero values, so adding a field can leave old
  hashes unchanged. The hash is 64-bit FNV, which is not cryptographic. Its
  v2 format replaced v1 to fix collisions. The repository was archived in
  July 2024.
- [`DeepHash`](https://zepworks.com/deepdiff/current/deephash.html), part of
  `deepdiff` (Python). Fields are excluded per call, by path (with
  wildcards), by type, or by a callback. Iterable order is ignored by
  default, and an option makes `10` and `10.0` hash the same. The default
  hash is SHA-256. Its documentation does not say whether hashes are stable
  across versions.

### Rust derives that skip fields from `std::hash::Hash`

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

[`digestible`](https://crates.io/crates/digestible) has an API similar to
`quiddity`'s: a derive, `#[digestible(skip)]`, `digest_with` for foreign
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

### Canonical serialization formats

A canonical format encodes each value as exactly one byte string, and a hash
of that string is a stable digest. These formats provide stability.
Selection comes from the format's skip attribute, which also removes the
field from the format, and allows one choice of fields per type.

- [`borsh`](https://borsh.io) (NEAR; "Binary Object Representation
  Serializer for Hashing"). A binary format with a written specification,
  implemented in Rust, JavaScript and other languages. The Rust crate has
  its own derive, with `#[borsh(skip)]`. It is maintained, widely used
  (including by Solana), sorts hash maps before encoding, and rejects NaN.
  Its derive is separate from serde's, so skipping a field from borsh does
  not affect JSON serialization. It supports one choice of fields per type.
  Its encoding is positional: adding, removing or reordering a field
  changes every digest, as does `u32` → `u64`. The Rust crate has no
  `serde_json::Value` impl; one was proposed and declined in favour of an
  example (near/borsh-rs#312, #317), so `Value` fields need
  `#[borsh(serialize_with = …)]`. That example encodes object keys in map
  order.
- [BCS](https://crates.io/crates/bcs) (Binary Canonical Serialization,
  from Diem; used by Aptos and Sui, whose SDKs implement it in other
  languages). In Rust it is driven by serde, so a field is skipped with
  `#[serde(skip)]`, which also removes it from JSON serialization. Aptos
  has derives in `aptos-core` (`CryptoHasher`, `BCSCryptoHash`) that prefix
  a per-type salt for domain separation. Positional, like borsh.
- [DAG-CBOR](https://ipld.io/specs/codecs/dag-cbor/spec/) (IPLD). The
  canonical CBOR used for content addressing in IPFS, implemented in Go,
  JavaScript, Rust and other languages. The Rust crate,
  [`serde_ipld_dagcbor`](https://crates.io/crates/serde_ipld_dagcbor), is
  serde-driven, with the same skip behaviour. Its content identifiers
  (CIDs) interoperate with the IPLD ecosystem.
- SSZ and its Merkle hash, as used by Ethereum consensus and implemented by
  each consensus client (Rust:
  [`ethereum_ssz`](https://crates.io/crates/ethereum_ssz) and
  [`tree_hash`](https://crates.io/crates/tree_hash)). A Merkle root allows
  proving one field without revealing the others. Designed for Ethereum's
  fixed schemas.

Protocol Buffers has a deterministic serialization mode, but its
documentation states that it is
[not canonical](https://protobuf.dev/programming-guides/serialization-not-canonical/):
the bytes can change with the schema, build flags or library version. It
recommends a fingerprint function that names the fields that matter.

Erlang's `erlang:phash2` gives the same hash for the same term on every
machine and ERTS version. It has no field selection, and its result has at
most 32 bits.

### JSON canonicalization (RFC 8785)

The JSON Canonicalization Scheme (JCS) specifies sorted object keys,
ECMAScript number formatting, and fixed string escaping. It encodes each
value as exactly one JSON string, which provides stability. It is
implemented in many languages, so any of them can recompute a digest. In
Rust, [`serde_jcs`](https://crates.io/crates/serde_jcs) and
[`serde_json_canonicalizer`](https://crates.io/crates/serde_json_canonicalizer)
implement it for serde values.

JCS specifies an encoding, not which fields are encoded. With serde,
selecting fields means `#[serde(skip)]`, which also changes the type's JSON
serialization. The common pattern is a second, borrowing struct holding the
fields that count, serialized with JCS and then hashed. Which fields count is
then recorded in that second struct, one per identity, and kept in step with
the original by hand. Adding a field changes every digest unless the field is
skipped when empty, and renaming a field always does.

### Schema-tolerant and structural hashing

- [`stable-hash`](https://crates.io/crates/stable-hash) (The Graph, Rust).
  Hashes are addressed by each field's position in the structure, a field
  at its default value contributes nothing (so adding an optional field
  leaves old hashes unchanged), and integers can widen. It provides
  stability and evolution. Its `impl_stable_hash!` macro destructures the
  struct, so every field must be listed: it has no skipping and no named
  identities. Its last release was in 2022.
- [objecthash](https://github.com/benlaurie/objecthash) (Ben Laurie). Hashes
  each field and combines the hashes, so that a digest can be checked with
  some fields redacted, and computed the same way from JSON, protobuf or
  other encodings. The original repository has implementations in Python,
  C, Go and Ruby, and there is a JavaScript port. The Rust ports,
  [`objecthash`](https://crates.io/crates/objecthash) and
  [`blot`](https://crates.io/crates/blot), have had no releases since 2017
  and 2018.
- [Ion Hash](https://amazon-ion.github.io/ion-hash/) (Amazon). A specified
  hash of Amazon Ion values, independent of whether they are stored as Ion
  text or binary. It is implemented in Java, JavaScript, Python, Go and
  .NET, and experimentally in Rust (`ion-rs`). It hashes whole values: it
  has no field selection and no domains, and Ion's types (such as integer,
  decimal and float) are part of the hash.
