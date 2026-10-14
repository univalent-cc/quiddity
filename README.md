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
