# Quiddity Specification

Version 1 — Draft

## 1. Introduction

### 1.1 Scope

This document specifies how a quiddity digest is computed. It has four
layers:

1. **Data model** (section 2): the values a digest is computed over.
2. **Encoding** (section 3): how a value is turned into bytes.
3. **Digests** (section 4): how bytes become a digest, how a digest is
   placed in a domain, shortened, and written as text.
4. **Rust mapping** (section 5): how Rust types, the derive macro and its
   attributes, and the crate's optional features map onto the data model.

Sections 2 to 4 are normative for every implementation, in any language.
Section 5 is normative for the Rust crate `quiddity` only. An
implementation in another language defines its own mapping onto the data
model and MUST then follow sections 2 to 4.

Design rationale is recorded in `DESIGN.md` and is not repeated here.

### 1.2 Conventions

The key words "MUST", "MUST NOT", "REQUIRED", "SHALL", "SHALL NOT",
"SHOULD", "SHOULD NOT", "RECOMMENDED", "MAY" and "OPTIONAL" in this document
are to be interpreted as described in BCP 14 (RFC 2119, RFC 8174) when, and
only when, they appear in all capitals.

Notation:

- `a ‖ b` is the concatenation of byte strings `a` and `b`.
- `0x1F` is a single byte. `[0x01, 0x02]` is a byte string.
- `u64be(n)` is the unsigned integer `n`, `0 ≤ n < 2^64`, as 8 bytes,
  most significant first.
- `utf8(s)` is the UTF-8 encoding of the string `s`.
- `ascii(s)` is the ASCII encoding of the string `s`.
- `SHA-256(x)` is the SHA-256 hash (FIPS 180-4) of the byte string `x`, a
  32-byte string.
- `E(v)` is the encoding of value `v` (section 3).

## 2. Data model

### 2.1 Values

A **value** is one of the following kinds:

| Kind | Values |
|---|---|
| null | the single value `null` |
| boolean | `false`, `true` |
| integer | any mathematical integer, of unbounded magnitude |
| float | any IEEE 754 binary64 value that is not a finite integer: a finite non-integral number, `+∞`, `−∞` or NaN |
| string | any finite sequence of Unicode scalar values |
| bytes | any finite sequence of bytes |
| list | any finite sequence of values |
| set | any finite multiset of values |
| map | any finite multiset of entries, each entry an ordered pair (key, value) of values |

Integer and float together are the **number** kinds.

A **set** is a multiset: it MAY contain values that are equal (2.4). A
**map** MAY contain several entries with equal keys. Sources whose own
semantics forbid such duplicates (a Rust `HashSet`, a JSON object parsed
with unique keys) never produce them; the multiset definition only makes
the encoding total and deterministic when a mapping onto the data model
makes distinct source values equal.

All NaN values are the same value in this data model. `+∞` and `−∞` are
distinct floats.

### 2.2 Number normalization

Before a number from a source is placed in the data model, it MUST be
normalized:

1. A binary64 value that is finite and whose mathematical value is an
   integer is the **integer** with that value. This includes `−0.0`, which
   is the integer 0, and values of any magnitude, such as `1e300`.
2. Any other binary64 value is a **float**: its value unchanged, except that
   every NaN is the single NaN of the data model.
3. A binary32 value is first converted exactly to binary64, then normalized
   by rules 1 and 2.
4. An integer from a source is the integer with that value, whatever its
   width or signedness in the source.

Consequently an integer and a float are never equal, and there is exactly
one data-model value for each number.

### 2.3 Strings

A string is a sequence of Unicode scalar values (code points U+0000 to
U+D7FF and U+E000 to U+10FFFF). An implementation MUST NOT produce a string
containing a surrogate code point. Where a source string may contain one (for
example, an ill-formed UTF-16 string), the implementation MUST reject it
rather than replace or drop the code point.

Strings are not normalized. Two strings that differ in their code points
are different values even if they are canonically equivalent under
Unicode normalization.

### 2.4 Equality

Two values are **equal** if and only if their encodings (section 3) are
identical byte strings.

### 2.5 Canonical order

Sets and maps are unordered. Their encodings (3.6, 3.7) arrange members and
entries in the **canonical order**, which is defined on encodings:

- Byte strings are compared lexicographically: at the first position where
  they differ, the one with the lesser byte, as an unsigned integer, is
  less; if one is a proper prefix of the other, the shorter is less.
- Set members are arranged by their encodings `E(m)`.
- Map entries are arranged by the encodings of their keys `E(k)`, and
  entries whose keys have identical encodings by the encodings of their
  values `E(v)`.
- Equal members, and entries equal in both key and value, are all retained;
  their relative order does not affect the encoding.

The canonical order depends only on the values, never on the type of
container holding them or its iteration order. It is not, in general,
numeric or alphabetical order: `-1` follows `1`, because the encoding of
`-1` begins with `0xFF`. (Informative: for strings that do not contain
U+0000, it coincides with code-point order.)

## 3. Encoding

### 3.1 Structure

Every value is encoded as its content, followed, for kinds of variable
size, by a length or count, followed by a one-byte **tag** identifying its
kind. Because each encoding ends with its tag, and a variable-size encoding's
extent is given by the fixed-width length or count before the tag, an
encoding can be parsed unambiguously from its last byte backwards. This
makes the encoding injective: distinct values have distinct encodings.

### 3.2 Tags

| Kind | Tag |
|---|---|
| null | `0x01` |
| boolean `false` | `0x02` |
| boolean `true` | `0x03` |
| integer | `0x10` |
| float | `0x11` |
| string | `0x20` |
| bytes | `0x21` |
| list | `0x30` |
| set | `0x31` |
| map | `0x32` |

Other tag values are reserved for future versions.

### 3.3 Null and boolean

```
E(null)  = [0x01]
E(false) = [0x02]
E(true)  = [0x03]
```

### 3.4 Numbers

**Integer.** Let `INT(n)` be the content octets of the ASN.1 DER encoding
of the INTEGER `n` (ITU-T X.690, 8.3): the shortest two's-complement
big-endian byte string representing `n`. In particular, `INT(0) = [0x00]`,
the first byte is never `0x00` followed by a byte with its high bit clear,
and never `0xFF` followed by a byte with its high bit set.

```
E(n) = INT(n) ‖ u64be(len(INT(n))) ‖ [0x10]
```

**Float.** Let `F(x)` be the 8-byte IEEE 754 binary64 interchange encoding
of `x`, big-endian, except that for NaN `F(NaN) = 0x7FF8000000000000`. In
particular `F(+∞) = 0x7FF0000000000000` and `F(−∞) = 0xFFF0000000000000`.

```
E(x) = F(x) ‖ [0x11]
```

A float is never a finite integer (2.2), so `F` is never applied to one.

### 3.5 Strings and bytes

```
E(s) = utf8(s) ‖ u64be(len(utf8(s))) ‖ [0x20]
E(b) = b ‖ u64be(len(b)) ‖ [0x21]
```

`len` is the length in bytes. Content MAY be produced incrementally (a
stream) before its length is known; the length is written after it.

### 3.6 Lists and sets

For a list `[v1, …, vn]`:

```
E(list) = E(v1) ‖ … ‖ E(vn) ‖ u64be(n) ‖ [0x30]
```

For a set with members `m1, …, mn` arranged in canonical order (2.5):

```
E(set) = E(m1) ‖ … ‖ E(mn) ‖ u64be(n) ‖ [0x31]
```

### 3.7 Maps

For a map with entries `(k1, v1), …, (kn, vn)` arranged in canonical order
(2.5):

```
E(map) = E(k1) ‖ E(v1) ‖ … ‖ E(kn) ‖ E(vn) ‖ u64be(n) ‖ [0x32]
```

`n` is the number of entries.

### 3.8 Limits

A length or count MUST be less than 2^64. An implementation MAY impose
lower limits on the size of values it accepts, and MUST report a value it
cannot encode rather than encode it differently.

## 4. Digests

### 4.1 Structural digest

The **structural digest** of value `v` is:

```
S(v) = SHA-256( ascii("quiddity/1/value") ‖ E(v) )
```

The prefix `ascii("quiddity/1/value")` is 16 bytes and identifies this
version of the specification.

### 4.2 Domains

A **domain label** is a non-empty string. The digest of a structural digest
`d` (32 bytes) in the domain labelled `L` is:

```
D(d, L) = SHA-256( ascii("quiddity/1/domain") ‖ utf8(L) ‖ u64be(len(utf8(L))) ‖ d )
```

The prefix `ascii("quiddity/1/domain")` is 17 bytes. The structural and
domain hash inputs begin with different prefixes, so no structural digest
input equals any domain digest input.

A value's **digest** is `S(v)` if it has no domain, and `D(S(v), L)` if it
has the domain `L`. A domain applies only to the value whose digest is
being computed; the encoding of a value nested within another value is
unaffected by any domain associated with the nested value.

`D` takes a full 32-byte structural digest. It is not defined on shortened
digests (4.3).

### 4.3 Shortened digests

A **128-bit digest** is the first 16 bytes of a digest. A **64-bit digest**
is the first 8 bytes. A shortened digest of a value in a domain is obtained
by placing the full structural digest in the domain first, then shortening.

### 4.4 Text forms

A digest is written as text as follows (RFC 5234 ABNF):

```
digest-text   = digest-256 / digest-128 / digest-64
digest-256    = "sha256:"     64HEXLOW
digest-128    = "sha256-128:" 32HEXLOW
digest-64     = "sha256-64:"  16HEXLOW
HEXLOW        = DIGIT / %x61-66        ; 0-9, a-f
```

The hex digits are the digest's bytes in order, each as two digits, most
significant first. ABNF string literals are case-insensitive; this document
overrides that for the prefixes, which MUST be lowercase. An implementation
MUST write hex digits in lowercase, and SHOULD accept uppercase hex digits
when parsing. A parser MUST reject text whose prefix does not match the
size of the digest being parsed.

### 4.5 Security considerations

- Digests are not secret, and the encoding is public: anyone can compute the
  digest of any value, in any domain. This specification provides no
  resistance to forgery. A domain fixes what a digest means, so that a
  signature over it, whose security comes from the signer's key, cannot be
  reused for a value of another kind.
- SHA-256's length-extension property is not a concern: it matters only
  when a secret is part of the hash input, and no secret ever is.
- Collision resistance is that of SHA-256 truncated to the digest's length:
  about 2^128 work for 256-bit digests, 2^64 for 128-bit digests, and
  2^32 for 64-bit digests. 64-bit digests MUST NOT be relied on where an
  adversary may choose values.

## 5. Rust mapping

This section is normative for the Rust crate `quiddity` only.

### 5.1 Identities

An **identity** selects, for a type, which fields are encoded and how. Every
type that implements `Quiddity` has the **primary** identity, represented by
the marker type `quiddity::Primary`. A type MAY declare further identities,
each represented by a zero-sized type implementing `IdentityMarker`. A type
encoded under identity `I` implements `Quiddity<I>`; `Quiddity` without a
parameter means `Quiddity<Primary>`.

An identity is not part of the hash input. It affects a digest only through
the encoding it produces. Two identities that produce the same encoding of
a value give that value the same digest.

`value.quiddity()` is the digest of `value` under `Primary`.
`value.quiddity_as::<I>()` is its digest under `I`. Both are computed as in
4.1 and 4.2, with the domain of the type's implementation for that identity,
if any.

### 5.2 Standard library types

Each of the following types implements `Quiddity<I>` for every identity `I`,
encoding its elements or fields, if any, under the same `I`.

| Rust type | Data-model value |
|---|---|
| `bool` | boolean |
| `i8` … `i128`, `isize`, `u8` … `u128`, `usize`, and their `NonZero` forms | integer |
| `f32`, `f64` | normalized by 2.2: integer or float |
| `char` | string of that one scalar value |
| `str`, `String`, `Box<str>`, `Rc<str>`, `Arc<str>`, `Cow<'_, str>` | string |
| `()` | null |
| `Option<T>` | `None`: null; `Some(x)`: the value of `x` |
| `[T; N]`, `[T]`, `Vec<T>`, `VecDeque<T>`, `Box<[T]>` | list, in order |
| tuples `(T1, …, Tn)`, `n ≥ 1` | list, in order |
| `BTreeSet<T>`, `HashSet<T, S>` | set |
| `BTreeMap<K, V>`, `HashMap<K, V, S>` | map |
| `&T`, `&mut T`, `Box<T>`, `Rc<T>`, `Arc<T>`, `Cow<'_, T>` | the value of `T` |
| `PhantomData<T>` | null; as a field of a derived type, absent (5.3) |

`Vec<u8>` and `[u8; N]` are lists of integers, not bytes. The bytes kind is
produced by `Encoder::bytes` and the `bytes` feature (5.6).

Known collapses, which follow from this table and match serde's JSON
representation:

- `Option<Option<T>>`: `Some(None)` and `None` are both null.
- `Option<()>`: `Some(())` and `None` are both null.

A `quiddity::Digest`, `Digest128` or `Digest64` used as a value is the
string of its text form (4.4).

Types requiring the `alloc` feature: everything above except primitives,
`char`, `str`, `()`, `Option`, arrays, slices, tuples, references and
`PhantomData`. Types requiring the `std` feature: `HashSet`, `HashMap`.

Every set and map type is encoded with its members or entries in canonical
order (2.5), whatever its own iteration order.

### 5.3 Derived implementations: structs

`#[derive(Quiddity)]` implements `Quiddity<Primary>`, and `Quiddity<I>` for
each `I` listed in `identities(…)`. For each identity, the **participating
fields** are the fields not skipped under that identity (5.5).

A field whose type is `PhantomData<T>` does not participate under any
identity, as if it were skipped; an attribute on it has no effect. A newtype
struct whose field is a `PhantomData<T>` is null, and a newtype variant whose
field is a `PhantomData<T>` has the value null.

| Struct form | Default encoding | Alternatives |
|---|---|---|
| named fields | map | `encode_as = list` |
| tuple struct with two or more fields | list | |
| newtype `struct S(T)` | the value of its field | |
| unit struct | null | |

- **Map.** One entry per participating field that is not omitted (5.5). The
  key is the string of the field's name, or of its `rename` for that
  identity. The value is the field's value. A struct encoded as a map is
  equal to any map with the same entries, including a JSON object.
- **List** (`encode_as = list`, or a tuple struct). The participating
  fields' values, in declaration order. A struct encoded as a list is equal
  to a tuple or list of the same values.
- **`transparent`.** The struct is encoded as the value of its single
  participating field. It is a compile error if, under any identity the
  attribute applies to, the number of participating fields is not exactly
  one.

It is a compile error if two participating fields have the same key under
the same identity.

### 5.4 Derived implementations: enums

| `encode_as` | Unit variant | Newtype variant | Tuple variant | Struct variant |
|---|---|---|---|---|
| `string` (default) | string: name | map `{name: value}` | map `{name: [values]}` | map `{name: {fields}}` |
| `int` | integer: discriminant | list `[discriminant, value]` | list `[discriminant, [values]]` | list `[discriminant, {fields}]` |

- The **name** is the variant's identifier, or its `rename`.
- The **discriminant** is the variant's Rust discriminant: the value written
  explicitly, or else one more than the previous variant's, the first being
  0.
- A struct variant's fields are encoded as a struct's (5.3), with the same
  field attributes.

### 5.5 Attributes

All attributes are written `#[quiddity(…)]`.

**Syntax and scoping.** Every attribute has the form
`name[(I1, …, In)] [= value]`. A flag written without a value means
`= true`. Without identities, an attribute applies to every identity of the
type. With identities, it applies to those listed and, for them, replaces
the unscoped form. `identities(…)` cannot be scoped. In identity lists,
`Primary` denotes `quiddity::Primary` without being imported. A marker type
named `Primary` is a compile error.

Type attributes:

| Attribute | Meaning |
|---|---|
| `identities(I, …)` | The type also implements `Quiddity<I>` for each listed `I`. |
| `domain = "L"` | The type's digests are in the domain `L` (4.2). `L` MUST be non-empty. |
| `transparent` | See 5.3. |
| `encode_as = map \| list` | Structs with named fields (5.3). |
| `encode_as = string \| int` | Enums (5.4). |
| `encode_as = display` | The value is the string produced by the type's `Display` impl. |
| `encode_as = serde` | The value is produced from the type's `Serialize` impl (5.7). Requires the `serde` feature. |

Field attributes:

| Attribute | Meaning |
|---|---|
| `skip` | The field does not participate. |
| `rename = "k"` | The field's key is `k`. |
| `identity = J` | The field's value is encoded under the field type's identity `J`. Without it, a field's value is encoded under the field type's primary identity, whatever identity the enclosing type is being encoded under. |
| `with = path` | The field's value is encoded by calling `path(&field, &mut Encoder)`. |
| `none_as_missing` | When the field is `None`, the field's entry is omitted. The field's type MUST be `Option<T>`; otherwise compile error. |
| `omit_if = path` | When `path(&field)` returns `true`, the field's entry is omitted. |
| `encode_as = display` | As the type attribute, for the field's value. |
| `encode_as = serde` | As the type attribute, for the field's value. |

Variant attributes: `rename = "name"` only.

**Omission.** An omitted field contributes no entry to the map, and is
not counted. A field is omitted when its `none_as_missing` or `omit_if`
condition holds under the identity being encoded. Each condition is
evaluated at most once per encoding. `none_as_missing` and `omit_if` on a
struct encoded as a list are compile errors.

**Applicability.** `encode_as = map | list` on anything but a struct with
named fields, and `encode_as = string | int` on anything but an enum, are
compile errors.

### 5.6 Optional features

| Feature | Types | Data-model value |
|---|---|---|
| `serde_json` | `Value`, `Map`, `Number` | see below |
| `indexmap` | `IndexMap<K, V, S>`, `IndexSet<T, S>` | map, set |
| `uuid` | `Uuid` | string: hyphenated, lowercase, e.g. `67e55044-10b1-426f-9247-bb680e5fe0c8` |
| `bytes` | `Bytes`, `BytesMut` | bytes |
| `url` | `Url` | string: `Url::as_str()` |

`serde_json::Value`:

| `Value` | Data-model value |
|---|---|
| `Null` | null |
| `Bool(b)` | boolean |
| `Number(n)` | number, as below |
| `String(s)` | string |
| `Array(a)` | list |
| `Object(m)` | map with string keys, whatever the iteration order of `m` |

A `Number` holding an `i64` or `u64` is that integer. A `Number` holding an
`f64` is normalized by 2.2.

With serde_json's `arbitrary_precision` feature, a `Number` holds its
decimal text. Such a `Number` MUST have the value it would have had without
the feature, so that a digest does not depend on a feature that any crate
in the dependency graph can enable: if its text is an integer within the
range of `i64` or `u64`, it is that integer; otherwise it is converted to
the nearest binary64 value (ties to even) and normalized by 2.2. A decimal
value that binary64 cannot represent exactly is therefore rounded, as JCS
(RFC 8785) and I-JSON (RFC 7493) also require.

### 5.7 The `serde` feature

`encode_as = serde` and `quiddity::serde::digest(&value)` encode a value
through its `Serialize` impl, using a serializer that maps serde's data
model as follows. The serializer reports `is_human_readable() == true`.

| serde | Data-model value |
|---|---|
| `bool` | boolean |
| `i8` … `i128`, `u8` … `u128` | integer |
| `f32`, `f64` | normalized by 2.2 |
| `char`, `str` | string |
| `bytes` | bytes |
| `none` | null |
| `some(v)` | the value of `v` |
| `unit`, `unit_struct` | null |
| `unit_variant` | string: the variant name |
| `newtype_struct(v)` | the value of `v` |
| `newtype_variant(v)` | map `{variant: v}` |
| `seq`, `tuple`, `tuple_struct` | list |
| `tuple_variant` | map `{variant: [values]}` |
| `map` | map |
| `struct` | map with the serialized fields' names as keys |
| `struct_variant` | map `{variant: {fields}}` |
| `skip_field` | no entry |

A `Serialize` impl may return an error. `quiddity::serde::digest` returns
it. Within `quiddity()` or `quiddity_as()`, which cannot return an error,
the error causes a panic.

## 6. Conformance

An implementation conforms to sections 2 to 4 if, for every value, it
produces the encoding of section 3 and the digests of section 4.

The normative test vectors are in `test-vectors/quiddity-v1.json`. The file
is generated by `test-vectors/generate.py`, using the Python implementation
of sections 2 to 4 in `test-vectors/qref.py`; the generator's header
describes the typed JSON notation for values. The file has four sections:

- `values`: a value, its encoding `E(v)` in hex, and its structural digest
  in each text form (4.4). Floats are given as binary64 bits before
  normalization, so they also test 2.2.
- `domains`: a value, a domain label, and the digest in that domain in each
  text form.
- `invalid_values`: values that an implementation MUST reject.
- `text_forms`: strings to be parsed as digest text, whether each is valid,
  and whether accepting or rejecting it is required (`MUST`) or recommended
  (`SHOULD`).

Where the vectors and the prose of sections 2 to 4 disagree, the prose is
normative and the vectors are in error. The examples in Appendix A are
informative and are included in the vectors.

## Appendix A. Examples (informative)

Each row gives a value, its encoding `E(v)` in hex, and its structural
digest `S(v)`.

| Value | `E(v)` | `S(v)` |
|---|---|---|
| `null` | `01` | `9b33f75d21a313761583c755e50288efa6ccb37c900dc413d3951395aada56c3` |
| `false` | `02` | `a4ed0a81c41082d0a594048e1d5d81ae85a74cfc9e5b4f2fc9d38f54b2c1aa12` |
| `true` | `03` | `1f7cd40876216c600adafc174c5cba0920d945af797c81b05591b288c1c21741` |
| `0` | `00 0000000000000001 10` | `1c6e2c7c95d2fd9b5bb439aac67a11822e9f3b28ee12899bf743818a389672c7` |
| `1` | `01 0000000000000001 10` | `2703bfd3b50d7dd6522d108837053d9af4e32055e94d0f69e63ca983bfac4f8c` |
| `-1` | `ff 0000000000000001 10` | `d51e197fd8fe34ec12c167f0c402bb17c0309fca56fb37e75d12cdc322f2b521` |
| `128` | `0080 0000000000000002 10` | `56cdc8560672bad80e52a006ea0d9ead51c1c601e8dce92e7ab20a52405c164d` |
| `-129` | `ff7f 0000000000000002 10` | `9c95bf53abed7ed40fe0ef7e877fd91fe7e5ae23c74e2ca3569474f42727f929` |
| binary64 `1.0` | (the integer 1) | `2703bfd3b50d7dd6522d108837053d9af4e32055e94d0f69e63ca983bfac4f8c` |
| binary64 `-0.0` | (the integer 0) | `1c6e2c7c95d2fd9b5bb439aac67a11822e9f3b28ee12899bf743818a389672c7` |
| `1.5` | `3ff8000000000000 11` | `abee116c081a17dae4b03f4ec6c3339cdb6134726180231494373b1c9ce377ee` |
| `+∞` | `7ff0000000000000 11` | `d455353696a0fe2ac9b62369b6eaa512135eeab8f0684dd1a7ad3d6364ee39df` |
| NaN | `7ff8000000000000 11` | `bce9627bba3659dc2f1d1a3fe58f45a7397ac56f06b6bb1e0c392c21d7b86cc0` |
| `"a"` | `61 0000000000000001 20` | `0c5e6442aeba354df983cdeb8d149971d02caa5e542a84f929c3dd7faa0d14a6` |
| bytes `0102` | `0102 0000000000000002 21` | `7991d5d6d37fb8a576ea776dc2fad7c9562867a447a84b9986093ced1210581b` |
| `[1, "a"]` | `01 0000000000000001 10 61 0000000000000001 20 0000000000000002 30` | `6d359306fa0b11cbc527ad722b2ad1a84ee1ff8e8da24d442e1778271cd0bd5e` |
| set `{2, 1}` | `01 0000000000000001 10 02 0000000000000001 10 0000000000000002 31` | `64406ae97b773d0d58df470287202a49cea075a3bd13fa3a7cd500b329d0bb08` |
| `{"start": 10, "end": 15}` | `656e64 0000000000000003 20 0f 0000000000000001 10 7374617274 0000000000000005 20 0a 0000000000000001 10 0000000000000002 32` | `ce5c58de07557be10cb78541577e3812bdb0b96b8af3aa17c4224753841bd989` |

Spaces in `E(v)` separate fields for readability and are not part of the
encoding. In the map example, the entry with key `"end"` precedes the entry
with key `"start"` by canonical order.

Domain example: for the map `{"to": "bob", "amount": 100}`,

```
S(v)                       = ebdada98fd6978a74c13d459b99036804651da3c7c677873973a497373f252b7
D(S(v), "example.payment") = f9104f8b85740d8c6c47c0f5a04bb05d264ee25f9be3d310be66385b298da830
128-bit                    = f9104f8b85740d8c6c47c0f5a04bb05d
64-bit                     = f9104f8b85740d8c
```

In text form, the last is `sha256-64:f9104f8b85740d8c`.
