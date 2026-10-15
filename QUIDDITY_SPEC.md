# Quiddity Specification

Version 1 — Draft

## 1. Introduction

### 1.1 Scope

This document specifies how a quiddity digest is computed. It has five
layers:

1. **Data model** (section 2): the values a digest is computed over.
2. **Encoding** (section 3): how a value is turned into bytes.
3. **Digests** (section 4): how bytes become a digest, how a digest is
   placed in a domain, shortened, and written as text.
4. **Identities** (section 5): which parts of a source value, such as a
   JSON document, are digested and how. An identity is itself a value,
   which passes between implementations in its JSON form.
5. **Rust mapping** (section 6): how Rust types, the derive macro and its
   attributes, and the crate's optional features map onto the data model
   and onto identities.

Sections 2 to 5 are normative for every implementation, in any language.
Section 6 is normative for the Rust crate `quiddity` only. An
implementation in another language defines its own mapping onto the data
model and MUST then follow sections 2 to 5.

Design rationale is recorded in `history/DESIGN.md` and is not repeated here.

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
- `utf8(s)` is the UTF-8 encoding of the text `s`.
- `ascii(s)` is the ASCII encoding of the string `s`.
- `SHA-256(x)` is the SHA-256 hash (FIPS 180-4) of the byte string `x`, a
  32-byte string.
- `E(v)` is the encoding of value `v` (section 3).

### 1.3 References

Normative:

- **[FIPS 180-4]** NIST, *Secure Hash Standard (SHS)*, FIPS PUB 180-4, 2015.
- **[IEEE 754]** IEEE, *Standard for Floating-Point Arithmetic*, IEEE Std
  754-2019.
- **[X.690]** ITU-T, *ASN.1 encoding rules: Specification of Basic Encoding
  Rules (BER), Canonical Encoding Rules (CER) and Distinguished Encoding
  Rules (DER)*, Recommendation X.690.
- **[POSIX]** IEEE Std 1003.1-2024, Base Definitions, *Seconds Since the
  Epoch*.
- **[RFC 2119]**, **[RFC 8174]** Key words for use in RFCs to indicate
  requirement levels.
- **[RFC 3629]** *UTF-8, a transformation format of ISO 10646*.
- **[RFC 5234]** *Augmented BNF for Syntax Specifications: ABNF*.
- **[RFC 5952]** *A Recommendation for IPv6 Address Text Representation*,
  section 4. Section 6.2 states its own rule for dotted notation.
- **[RFC 8259]** *The JavaScript Object Notation (JSON) Data Interchange
  Format*.

Informative:

- **[RFC 7493]** *The I-JSON Message Format*.
- **[RFC 8785]** *JSON Canonicalization Scheme (JCS)*. Appendix B compares
  it with this specification.
- **[WTF-8]** S. Sapin, *The WTF-8 encoding*,
  <https://simonsapin.github.io/wtf-8/>. Section 2.3 states the part of
  WTF-8 that this specification uses.

## 2. Data model

### 2.1 Values

A **value** is one of the following kinds:

| Kind | Values |
|---|---|
| null | the single value `null` |
| boolean | `false`, `true` |
| integer | any mathematical integer, of unbounded magnitude |
| float | any IEEE 754 binary64 value that is not a finite integer: a finite non-integral number, `+∞`, `−∞` or NaN |
| string | any finite sequence of bytes; text is placed in the data model as bytes (2.3) |
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

A string is a sequence of bytes. There is no separate kind for text: text
and binary data are both strings, and are equal when their bytes are equal.

- Binary data is its bytes.
- Text that is a sequence of Unicode scalar values (code points U+0000 to
  U+D7FF and U+E000 to U+10FFFF) is its UTF-8 encoding.
- Text that may contain surrogate code points, such as an ill-formed UTF-16
  string, is its WTF-8 encoding [WTF-8]: each high surrogate immediately followed
  by a low surrogate is first combined into the scalar value the pair
  represents, and each remaining surrogate code point is written as the
  three bytes that UTF-8's encoding scheme gives that code point (for
  U+D800, `[0xED, 0xA0, 0x80]`). For text without surrogates, this is the
  same as UTF-8.

Text is not normalized. Two texts that differ in their code points are
different strings even if they are canonically equivalent under Unicode
normalization.

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
`-1` begins with `0xFF`. (Informative: for strings that do not contain the
byte `0x00`, it coincides with byte order, which for UTF-8 text is
code-point order.)

### 2.6 Times and durations

The data model has no kind for times or durations. So that digests agree
across languages, an implementation that maps times or durations onto the
data model SHOULD map them as follows:

- An **instant** is the integer number of nanoseconds since
  1970-01-01T00:00:00 UTC (the Unix epoch), negative for an instant before
  it. Time is counted as in POSIX [POSIX]: every day has 86 400 seconds,
  and leap seconds are not counted.
- A **duration** is the integer number of nanoseconds, negative for a
  negative duration.

The conversion MUST be exact. It MUST NOT pass through binary floating
point, which cannot hold a current instant to the nanosecond. A source
value of coarser precision, such as microseconds, is multiplied exactly by
its unit, so an instant has the same integer whatever precision its source
keeps. For example, 2024-01-01T00:00:00Z is `1704067200000000000`, and
1969-12-31T23:59:59Z is `-1000000000`.

A date and time without a time zone or UTC offset (a "local" or "naive"
date and time) is not an instant, and a duration in calendar units, such as
months, has no fixed number of nanoseconds. Neither has a mapping here.

### 2.7 JSON

So that a JSON document has the same digest in every language, an
implementation that maps JSON [RFC 8259] onto the data model SHOULD map it
as follows:

| JSON | Data-model value |
|---|---|
| `null` | null |
| `true`, `false` | boolean |
| number | as below |
| string | string: its text (2.3), after escapes are decoded |
| array | list, in order |
| object | map with string keys; member order does not matter |

A number whose text has no fraction and no exponent, and whose value is
from −2^63 to 2^64 − 1, is that integer. Any other number is converted to
the nearest binary64 value (ties to even) and normalized by 2.2. So `1`,
`1.0` and `1e0` are the integer 1, and `9007199254740993` is that integer,
while `9007199254740993.0` is the binary64 value 2^53, the integer
9007199254740992. The bounds are those of 64-bit integers, which most JSON
parsers represent exactly.

This mapping is defined for objects without duplicate names, as I-JSON
[RFC 7493] requires.

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

### 3.5 Strings

```
E(s) = s ‖ u64be(len(s)) ‖ [0x20]
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

A **domain label** is a non-empty string (2.3), usually text. The digest of
a structural digest `d` (32 bytes) in the domain labelled `L` is:

```
D(d, L) = SHA-256( ascii("quiddity/1/domain") ‖ L ‖ u64be(len(L)) ‖ d )
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

## 5. Identities

*Draft. Implemented in `test-vectors/qident.py` and, for JSON values, in
the Rust crate's `identity` feature.*

### 5.1 Overview

An **identity** describes how a value from a source, such as a parsed JSON
document, becomes a data-model value: which members of an object count,
under which keys, what kind each value has, and how it is converted. An
identity is a data structure of nested maps, lists and strings (5.3).
Its JSON form, in which this section writes it, lets every implementation
read and write the same identity.

Applying an identity to a source value gives a data-model value, or an
error if the source value does not have the shape the identity describes.
The **digest of a source value under an identity** is the digest (4.1) of
that data-model value, in the identity's domain (4.2) if it has one.

Section 6 describes how Rust types define identities statically and how a
Rust type exports them as identities (6.8). An implementation in a language
without static types MAY let an identity hold functions as extensions; an
identity that holds one has no JSON form and no digest (5.10).

### 5.2 Source values

A source value has one of these **source types**:

- the JSON types (RFC 8259): **null**, **boolean**, **number**,
  **string**, **array** and **object**. A JSON number's value is the one
  2.7 gives it. An object's members are its keys, which are strings, and
  their values.
- for sources other than JSON, which can hold more: **bytes**, **set**,
  **instant** and **duration**, and objects whose keys are of any source
  type.

An implementation reading a source other than JSON maps each of its values
to one of these source types. A value that is already a data-model value of
a kind's result (such as bytes for `bytes`, or an instant for `instant`) is
taken as it is.

### 5.3 Identities

An identity is a map with these members. In its JSON form it is a JSON
object (I-JSON, RFC 7493), and each member's value is written with the
JSON type its description names: an integer is a JSON number written
without a fraction or exponent.

| Member | Required | Value |
|---|---|---|
| `root` | yes | The kind (5.4) of the value being digested. |
| `domain` | no | A non-empty string: the domain (4.2) of digests under this identity. |
| `definitions` | no | An object mapping names (non-empty strings) to kinds, which `ref` kinds refer to. |

Example:

```json
{
  "domain": "example.question",
  "root": {"ref": "Question"},
  "definitions": {
    "Question": {"object": {"fields": {
      "brief": {"kind": "string"},
      "rows": {"kind": {"list": {"ref": "Row"}}},
      "tags": {"kind": {"set": "string"}, "omit": ["empty"]},
      "note": {"kind": {"union": ["null", "string"]}, "omit": ["none"]},
      "asked_at": {"kind": {"instant": {}}},
      "kind": {"kind": "string", "rename": "type"}
    }}},
    "Row": {"object": {"fields": {
      "label": {"kind": "string"},
      "value": {"kind": "number"}
    }}}
  }
}
```

Every definition lives at the identity's top level, and every `ref`
resolves there; a `ref` to a name that is not defined is an error.

An implementation MUST reject an identity with a member, or a string value
in a place that takes one of a fixed set of strings, that this section does
not define. Ignoring one would change digests without notice. For the same
reason an identity has no version: one that uses a member defined by a
later version of this section is rejected by an implementation of an
earlier one.

Optional members have defaults. An implementation MUST treat an optional
member whose value is its default as absent. The **canonical form** of an
identity omits every such member, and the members `domain` and
`definitions` when they are absent or empty; it omits every definition
that the root does not reach, directly or through other definitions; and
its unions are flattened and merged (5.7). A definition that is not
reached is still validated (5.9).

### 5.4 Kinds

A kind is written either as a string, for kinds without options, or as an
object with exactly one member, whose name is the kind and whose value
gives its options.

Each kind accepts some source types **natively** and others **by
conversion** (5.5). A source value of any other type is an error.

| Kind | Written | Native | By conversion | Data-model value |
|---|---|---|---|---|
| null | `"null"` | null | | null |
| bool | `"bool"` | boolean | string | boolean |
| integer | `"integer"` | number | string | integer |
| number | `"number"` | number | string | integer or float (2.2) |
| string | `"string"` | string | | string |
| uuid | `"uuid"` | string | | string: the canonical form (5.5) |
| path | `"path"` | string | object | string: the path's bytes (5.5) |
| bytes | `{"bytes": {"text": "hex"}}` | bytes | array, string | string: the bytes |
| instant | `{"instant": {"unit": u}}` | instant | string, object; number if `unit` | integer: nanoseconds (2.6) |
| duration | `{"duration": {"unit": u}}` | duration | string, object; number if `unit` | integer: nanoseconds (2.6) |
| list | `{"list": k}` | array | | list of `k` |
| set | `{"set": k}` | array, set | | set of `k` |
| tuple | `{"tuple": [k1, …, kn]}` | array | | list |
| map | `{"map": {"key": k, "value": k}}` | object | | map |
| object | `{"object": {…}}` | object | | map or list (5.6) |
| union | `{"union": [k1, …, kn]}` | (5.7) | (5.7) | the chosen alternative's value |
| tagged | `{"tagged": {…}}` | (5.8) | | (5.8) |
| ref | `{"ref": "name"}` | as the definition | as the definition | as the definition |
| any | `"any"` | every type | | the source value as it is: by 2.7 for JSON |

Options:

- `bytes`: `text` is `"base64"` (the default) or `"hex"`: how a string
  source is read.
- `instant`, `duration`: `unit` is `"s"`, `"ms"`, `"us"` or `"ns"`: the
  unit of a number source. Without `unit`, a number source is an error.
  Both kinds are written as objects even without options: `{"instant": {}}`.
- `tuple`: a non-empty array of kinds; the source array MUST have that
  many items, each read with the kind in the same position.
- `map`: the source object's keys are read with `key`, and its values with
  `value`. In a JSON source every key is a string, so `key` MUST accept
  strings, natively or by conversion; if it is a union, at most one of its
  alternatives may accept strings.
- `list` and `tuple` do not accept a set source, which has no order.

### 5.5 Conversions

A conversion recognizes a value of the kind written in another form, such
as an instant written as text; each accepted form denotes exactly one
value. No conversion renders a value as text: `string` accepts only
strings, since rendering `true` or `5` would choose a spelling (`"True"`,
`"5.0"`) that the source did not.

Before a string source is converted, ASCII whitespace (U+0009, U+000A,
U+000D, U+0020) is removed from both ends. "Decimal text" below is a
number in the syntax of RFC 8259, section 6, optionally preceded by `+`.

- **bool** from a string, matched case-insensitively: `true`, `t`, `yes`,
  `y`, `on` and `1` are true; `false`, `f`, `no`, `n`, `off` and `0` are
  false.
- **integer** from a number: the number's data-model value, which MUST be
  an integer. From a string: decimal text whose value is an integer, such
  as `5`, `5.0` or `1e3`, as that exact integer, of any size.
- **number** from a string, matched case-insensitively: `nan`, `+nan` and
  `-nan` are NaN; `inf`, `+inf`, `infinity`, `+infinity`, `∞` and `+∞` are
  `+∞`; `-inf`, `-infinity` and `-∞`, and the same with U+2212 `−` in
  place of `-`, are `−∞`. Decimal text is the value 2.7 gives that text as
  a JSON number.
- **uuid** from a string, case-insensitively: 32 hexadecimal digits, either
  in groups of 8, 4, 4, 4 and 12 separated by `-` or ungrouped; either form
  enclosed in `{` and `}`; or the grouped form preceded by `urn:uuid:`.
  The canonical form is the grouped form in lowercase.
- **path**: a file system path or operating system string. A string
  source is its UTF-8 bytes. From an object, as serde writes Rust's
  `OsStr` and `OsString`: exactly one member, either `Unix` with an array
  of integers from 0 to 255, which are the bytes, or `Windows` with an
  array of integers from 0 to 65 535, which are UTF-16 code units and are
  converted to WTF-8 (2.3). A path that is valid Unicode text is therefore
  the same string whichever form it comes in. Paths are not normalized.
- **bytes** from an array: each item MUST be a number whose value is an
  integer from 0 to 255. From a string with `text` `"hex"`: an even number
  of hexadecimal digits, in either case. From a string otherwise: base64
  (RFC 4648), in the standard alphabet (section 4) or the URL-safe one
  (section 5) but not both, with padding either correct or absent, and no
  other characters.
- **instant** from a string: a date-time in the syntax of RFC 3339, section
  5.6, with these allowances: the separator between date and time may be
  `T`, `t` or a space; the offset may be `Z`, `z`, `±hh:mm`, `±hhmm` or
  `±hh`; the fraction of a second has 1 to 9 digits. A second of `60` (a
  leap second) is an error, as it is in the standard date-time parsers of
  Python, JavaScript, Go and Elixir, which cannot represent one. A
  date-time without an offset is an error (2.6). The result is the
  instant's nanoseconds since the Unix epoch. With `unit`, a string that is
  decimal text is read instead as that number of units. From an object: exactly the members `secs_since_epoch` (an integer,
  at least 0) and `nanos_since_epoch` (an integer from 0 to 999 999 999),
  as serde writes Rust's `SystemTime`; the result is
  `secs_since_epoch × 10^9 + nanos_since_epoch`.
- **duration** from a string: an ISO 8601 duration, optionally preceded by
  `+` or `-`: `P`, then any of `nW` and `nD`, then optionally `T` and any of
  `nH`, `nM` and `n.fS`, with at least one component, designators in either
  case, whole-number counts, and a fraction of 1 to 9 digits on seconds
  only. A week is 7 days and a day 86 400 seconds. Years and months, which
  have no fixed length, are an error. With `unit`, a string that is decimal
  text is read instead as that number of units. From an object: exactly the
  members `secs` (an integer, at least 0) and `nanos` (an integer from 0 to
  999 999 999), as serde writes Rust's `Duration`.
- **instant** and **duration** from a number, or from decimal text, with
  `unit`: the value times the unit MUST be a whole number of nanoseconds,
  computed exactly (a string is read as an exact decimal; a number as its
  data-model value).

An implementation MAY limit the size of values it converts, such as the
exponent of decimal text, and MUST report a value it cannot convert as an
error (3.8).

### 5.6 Objects

`{"object": {…}}` has these members:

| Member | Required | Value |
|---|---|---|
| `fields` | yes | An object mapping each field's name to the field (below). It may be empty. |
| `unlisted` | no | What happens to source members whose names are not in `fields`: `"skip"` (the default), they are ignored. No other value is defined yet. |
| `order` | no | An array naming every field exactly once: the value is then the list of the fields' values in this order, rather than a map. |

A field has these members:

| Member | Required | Value |
|---|---|---|
| `kind` | yes | The field's kind. |
| `rename` | no | The field's key in the result, if not its name. |
| `omit` | no | An array of conditions, each `"none"` or `"empty"`, each at most once: the field's entry is left out when its value is null (`"none"`), or an empty string, list, set or map (`"empty"`). Its order does not matter. |
| `quantize` | no | A positive integer `q`: the field's value, an integer, instant or duration, is quantized to a multiple of `q` (in nanoseconds for instants and durations); see below. `1` is the default. |
| `decimals` | no | An integer `d` from −300 to 300: the field's value, a number, is quantized to a multiple of `10^−d`; see below. |

To apply an object kind to a source object:

1. For each field, the source value is the member with the field's name,
   or null if there is none. It is read with the field's kind, and then
   quantized, if the field says so.
2. The field is omitted if its value meets one of its `omit` conditions.
3. Without `order`, the result is the map from each field's key (its
   `rename`, or else its name) to its value, for the fields not omitted.
   With `order`, it is the list of the fields' values in that order.

A missing member is read as null, so a field whose kind does not accept
null (such as `"string"`) makes a missing member an error; `{"union":
["null", "string"]}` accepts it. This is the static rule for `Option`
fields (6.3, 6.5).

**Quantization.** A field with `quantize` or `decimals` maps its value onto
a grid, so that values differing only below the grid's step have the same
digest. The value `v` becomes the greatest multiple of the step that is not
greater than `v` (it is rounded toward negative infinity), computed
exactly:

- With `quantize` `q`, an integer `v` becomes `⌊v / q⌋ × q`; an instant or
  duration, as integer nanoseconds (2.6), likewise.
- With `decimals` `d`, a number `v` becomes `⌊v × 10^d⌋ × 10^−d`, computed
  exactly on the decimal value of `v`: an integer's own value, or, for a
  float, the shortest decimal that rounds to it (among the shortest, the
  one nearest its exact value), which is the decimal a JSON writer emits.
  The result is the integer it equals, or else the binary64 value nearest
  it, normalized by 2.2. NaN and the infinities are unchanged. With `d`
  negative, the step is `10^|d|`.
- Null is unchanged.

A value already on the grid is unchanged, so quantization does not alter
values that already conform. For example, with `quantize` 100, 51 and 99
become 0 and 100 and 149 become 100, and −1 becomes −100; with `decimals`
2, 12.989 becomes 12.98, 2.675 becomes 2.67, and 2.67 stays 2.67.

(Informative: a float's exact binary64 value is not used, because a
decimal such as 2.67 is usually stored slightly above or below its written
value, and rounding down the value stored for 2.67, which is slightly less,
would give 2.66. The shortest decimal is the one Python's `repr`,
JavaScript's `Number.prototype.toString`, Go's `strconv.FormatFloat` with
precision −1 and Rust's `Display` produce.)

`quantize` is allowed only on a field whose kind is `"integer"`,
`{"instant": …}` or `{"duration": …}`, and `decimals` only on one whose
kind is `"number"`; either kind may also be the one alternative besides
`"null"` of a union, or reached through a `ref`. A field has at most one of
them.

### 5.7 Unions

`{"union": [k1, …, kn]}`, with one or more alternatives, accepts a value
that one of its alternatives accepts. The order of the alternatives does
not matter.

An alternative **claims** the source types it accepts natively, and those
it accepts by conversion (5.4); a `ref` claims what its definition claims,
and a nested union what its alternatives claim. Nested unions are
flattened into the union containing them, and alternatives that are equal
in canonical form are merged; a union left with one alternative is that
alternative, so `["null", "null"]`, the export of `Option<()>`, is
`"null"`. A union is valid only if no source type is
claimed natively by two alternatives, and none is claimed by conversion by
two alternatives.

A source value is read with the alternative that claims its source type
natively, or, if none does, with the one that claims it by conversion. The
result is that alternative's value; no tag is added. So under
`{"union": ["integer", "string"]}`, the number `5` is the integer 5 and the
string `"5"` is the string `"5"`; under `{"union": ["bool", "string"]}`,
the string `"yes"` is the string `"yes"`, while under `"bool"` alone it is
`true`. A conversion applies only where no alternative accepts the value
as it is.

**Objects told apart by their keys.** As an exception to the rule above,
several alternatives may accept objects natively if each of them is an
object kind (5.6) or a `ref` to one, and no other alternative accepts
objects natively. A field of such an alternative is **required** if its
kind does not accept null. A source object **fits** an alternative if each
of its members' names is the name of one of the alternative's fields, and
each of the alternative's required fields is a member. The union is valid
only if no source object can fit two of these alternatives: for every two
of them, `A` and `B`, some required field of `A` is not a field of `B`, or
some required field of `B` is not a field of `A`. A source object is read
with the alternative it fits; if it fits none, it is read with the
alternative that accepts objects by conversion, if there is one, and is
otherwise an error. This is how an untagged union of object types, common
in TypeScript, is told apart: by which keys are present, never by trying
alternatives in order.

Valid: `["bool", "string"]`, `["integer", "string"]`, `["null", k]` for
any `k` that does not accept null, `["string", {"ref": "Row"}]` where `Row`
is an object, and objects with the fields `{x, y}` and `{w, h}`. Not
valid: two lists, `"string"` and `"uuid"`, `"any"` with anything, an object
and a map, objects with the same fields, and objects with the fields
`{x, y}` and `{x, y, z}` where `z` accepts null (an object with `x` and `y`
fits both). An object that names its variant in a member is a tagged union
(5.8).

### 5.8 Tagged unions

`{"tagged": {…}}` describes a value that is one of several named variants,
in one of the forms serde writes Rust enums in. It has these members:

| Member | Required | Value |
|---|---|---|
| `variants` | yes | A non-empty object mapping each variant's name to the variant (below). |
| `style` | no | `"external"` (the default), `"internal"` or `"adjacent"`. |
| `tag` | for `internal` and `adjacent` | The name of the member holding the variant's name. |
| `content` | for `adjacent` | The name of the member holding the variant's value. It differs from `tag`. |
| `encode_as` | no | `"string"` (the default): a variant is identified in the result by its name; `"int"`: by its discriminant. |

A variant has these members:

| Member | Required | Value |
|---|---|---|
| `kind` | no | The variant's kind. A variant without one is a unit variant. |
| `rename` | no | The variant's name in the result, if not its name. |
| `discriminant` | with `"int"` only | An integer from −2^63 to 2^64 − 1, different for every variant. |

In the source, a variant is always identified by its name. In the result,
it is identified by its **label**: its `rename` or else its name, as a
string, or with `"int"` its discriminant, as an integer.

| Style | Source, unit variant | Source, other variant | Result, unit variant | Result, other variant |
|---|---|---|---|---|
| external | the string `name` | an object with exactly one member, `name: v` | the label | `{label: v}`, or with `"int"` the list `[label, v]` |
| internal | an object with the member `tag: name` | an object with `tag: name` and the variant's other members | `{tag: label}` | the variant's object value, with the entry `tag: label` added |
| adjacent | an object with `tag: name` | an object with `tag: name` and `content: v` | `{tag: label}` | `{tag: label, content: v}` |

`v` is the source value read with the variant's kind. With `internal`, a
variant's kind MUST be an object kind, or a `ref` to one, that has no
`order` and no field named `tag`; it is applied to the source object
without its `tag` member. Other members of the source object are ignored
with `internal` and `adjacent`, as unlisted members are.

A tagged union claims natively: with `external`, strings if it has a unit
variant and objects if it has another; with `internal` and `adjacent`,
objects.

### 5.9 Validation

When an implementation reads an identity, it MUST reject it if:

- a member or string value is not defined by this section (5.3), or a
  required member is missing;
- a member's value is not of the type its description names, such as an
  integer written as `5.0`;
- `domain` is empty;
- a `ref` names no definition, or a chain of `ref`s leads back to its start
  without passing through a `list`, `set`, `tuple`, `map`, `object` or
  `tagged` kind;
- a union is not valid (5.7), including object alternatives that a
  source object could fit two of, or a map's `key` does not accept
  strings (5.4);
- two fields of an object have the same key, an object has both `order` and
  a field with `omit`, or `order` does not name every field exactly once;
- a field has `quantize` that is not a positive integer, `decimals` that is
  not an integer from −300 to 300, both, or either on a kind it does not
  apply to (5.6);
- two variants have the same label, `tag` or `content` is missing or given
  where not allowed, `discriminant` is missing or given where not allowed,
  or an internally tagged variant's kind is not as 5.8 requires.

### 5.10 The digest of an identity

The digest of an identity is the digest, under the identity in Appendix C,
of the identity in canonical form (5.3). Appendix C's identity has the
domain `quiddity.identity`, and it describes itself: its digest is
computed with itself.

Two identities that differ only in the order of members, in the order of a
union's alternatives or of `omit` conditions, or in optional members given
at their defaults, have the same digest.

## 6. Rust mapping

This section is normative for the Rust crate `quiddity` only.

### 6.1 Identities

An **identity** selects, for a type, which fields are encoded and how
(section 5 describes identities in general). Every
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

### 6.2 Standard library types

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
| `[T; N]`, `[T]`, `Vec<T>`, `VecDeque<T>`, `LinkedList<T>`, `Box<[T]>` | list, in order |
| tuples `(T1, …, Tn)`, `n ≥ 1` | list, in order |
| `BTreeSet<T>`, `HashSet<T, S>` | set |
| `BTreeMap<K, V>`, `HashMap<K, V, S>` | map |
| `&T`, `&mut T`, `Box<T>`, `Rc<T>`, `Arc<T>`, `Cow<'_, T>`, `Pin<P>` | the value of `T` (for `Pin<P>`, of `P`'s target); at the top level, in its domain, if any |
| `Wrapping<T>`, `Saturating<T>`, `Reverse<T>`, `Cell<T>` (`T: Copy`) | the value of `T` |
| `AtomicBool`, `AtomicI8` … `AtomicI64`, `AtomicIsize`, `AtomicU8` … `AtomicU64`, `AtomicUsize` | the loaded value |
| `PhantomData<T>` | null; as a field of a derived type, absent (6.3) |
| `Result<T, E>` | `Ok(x)`: map `{"Ok": x}`; `Err(e)`: map `{"Err": e}` |
| `Infallible` | (has no values) |
| `Range<T>`, `RangeInclusive<T>` | map `{"start": a, "end": b}` |
| `RangeFrom<T>` | map `{"start": a}` |
| `RangeTo<T>`, `RangeToInclusive<T>` | map `{"end": b}` |
| `Bound<T>` | `Included(x)`: map `{"Included": x}`; `Excluded(x)`: map `{"Excluded": x}`; `Unbounded`: string `Unbounded` |
| `Duration` | integer: nanoseconds (2.6) |
| `SystemTime` | integer: nanoseconds since the Unix epoch, negative before it (2.6) |
| `IpAddr`, `Ipv4Addr`, `Ipv6Addr` | string: the address as text, as below |
| `SocketAddr`, `SocketAddrV4`, `SocketAddrV6` | string: the socket address as text, as below |
| `OsStr`, `OsString`, `Path`, `PathBuf` | string: the platform's bytes, as below |
| `CStr`, `CString` | string: the bytes before the terminating NUL |

`Vec<u8>` and `[u8; N]` are lists of integers, as they are in serde's JSON.
Binary data is written as a string by `Encoder::bytes` and the `bytes`
feature (6.6).

Through the `serde` feature (6.7), a value is encoded as its `Serialize`
impl writes it, which can differ from this table. For example, serde
writes a `Duration` as the map `{"secs": s, "nanos": n}`.

**Addresses.** An IPv4 address is in dotted decimal, such as `192.0.2.1`.
An IPv6 address is in the text form of [RFC 5952], section 4: lowercase hex
digits without leading zeros, the longest run of two or more zero groups
written `::` (the first, among equally long runs), and no other
compression. An IPv4-mapped address (`::ffff:0:0/96`) has its last 32 bits
in dotted decimal, such as `::ffff:192.0.2.1`; no other address does, so
`::192.0.2.1` is `::c000:201`. A socket address is `a:p` for an IPv4
address `a` and port `p`, and `[a]:p` for an IPv6 address, or `[a%s]:p`
with its scope ID `s` in decimal if that is not 0. The IPv6 flow label is
not encoded.

**Platform strings.** On Unix, an `OsStr` is its bytes. On Windows, it is
the WTF-8 encoding (2.3) of its UTF-16 code units. On other platforms, it
is the bytes of `OsStr::as_encoded_bytes`. On every platform, a path that
is valid Unicode text is therefore its UTF-8 bytes, the same string as a
`str` holding the same text. Paths are not normalized: `a/b`, `a//b`,
`./a/b` and `a\b` are different strings.

Known collapses, which follow from this table and match serde's JSON
representation:

- `Option<Option<T>>`: `Some(None)` and `None` are both null.
- `Option<()>`: `Some(())` and `None` are both null.

A `quiddity::Digest`, `Digest128` or `Digest64` used as a value is the
string of its text form (4.4).

Types requiring the `std` feature: `HashSet`, `HashMap`, `SystemTime`,
`OsStr`, `OsString`, `Path` and `PathBuf`. Types requiring the `alloc`
feature: `String`, `Box`, `Rc`, `Arc`, `Cow`, `Vec`, `VecDeque`,
`LinkedList`, `BTreeSet`, `BTreeMap` and `CString`. The other types need
neither. An atomic type is implemented on targets that support atomic
operations of its width.

Every set and map type is encoded with its members or entries in canonical
order (2.5), whatever its own iteration order.

### 6.3 Derived implementations: structs

`#[derive(Quiddity)]` implements `Quiddity<Primary>`, and `Quiddity<I>` for
each `I` listed in `identities(…)`. For each identity, the **participating
fields** are the fields not skipped under that identity (6.5).

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

- **Map.** One entry per participating field that is not omitted (6.5). The
  key is the string of the field's name, or of its `rename` for that
  identity. The value is the field's value. A struct encoded as a map is
  equal to any map with the same entries, including a JSON object.
- **List** (`encode_as = list`, or a tuple struct). The participating
  fields' values, in declaration order. A struct encoded as a list is equal
  to a tuple or list of the same values.
- **`transparent`.** The struct is encoded as the value of its single
  participating field. It is a compile error if, under any identity the
  attribute applies to, the number of participating fields is not exactly
  one. If that field's type is generic, instantiating it as `PhantomData`
  is a compile error where the instantiated type is digested.

It is a compile error if two participating fields have the same key under
the same identity. A newtype struct whose field is skipped under an
identity is null under that identity. A unit struct, and a struct with no
fields, is null.

### 6.4 Derived implementations: enums

| `encode_as` | Unit variant | Newtype variant | Tuple variant | Struct variant |
|---|---|---|---|---|
| `string` (default) | string: name | map `{name: value}` | map `{name: [values]}` | map `{name: {fields}}` |
| `int` | integer: discriminant | list `[discriminant, value]` | list `[discriminant, [values]]` | list `[discriminant, {fields}]` |

- The **name** is the variant's identifier, or its `rename`.
- The **discriminant** is the variant's Rust discriminant: the value written
  explicitly, or else one more than the previous variant's, the first being
  0.
- A struct variant's fields are encoded as a struct's (6.3), with the same
  field attributes.

**Tagging.** Without `tag`, `content` or `untagged`, an enum is externally
tagged, as in the table above. The other styles are serde's, and an enum
with the same serde attributes has the same digest through the `serde`
feature (6.7). The **label** of a variant is its name, or with
`encode_as = int` its discriminant.

| Attributes | Unit variant | Struct variant | Newtype and tuple variants |
|---|---|---|---|
| `tag = "t"` (internal) | map `{t: label}` | the map of its fields, with the entry `t: label` added | compile error |
| `tag = "t", content = "c"` (adjacent) | map `{t: label}` | map `{t: label, c: {fields}}` | map `{t: label, c: value}`, the value as in the table above |
| `untagged` | null | map `{fields}` | the value as in the table above |

It is a compile error if: `content` is given without `tag`, or equals it;
`untagged` is combined with `tag`, `content` or `encode_as = int`; under
internal tagging, a field's key equals the tag; or, under `untagged`, two
variants are unit variants, struct variants with the same keys, tuple
variants with the same number of fields, or newtype variants of the same
type, since nothing would tell them apart. Struct variants with different
keys are told apart by their keys, as objects in a union are (5.7);
variants that the syntax does not show to overlap are checked when the
identity is exported (6.8).


### 6.5 Attributes

All attributes are written `#[quiddity(…)]`.

**Syntax and scoping.** Every attribute has the form
`name[(I1, …, In)] [= value]`. A flag written without a value means
`= true`. Without identities, an attribute applies to every identity of the
type. With identities, it applies to those listed and, for them, replaces
the unscoped form. `identities(…)` cannot be scoped. In identity lists,
`Primary` denotes `quiddity::Primary` without being imported. A marker type
named `Primary` is a compile error. Identities in scope lists are matched by
name, the last segment of their path with any generic arguments, so
`skip(Meaning)` and `skip(crate::Meaning)` name the same identity; it is a
compile error for two identities in `identities(…)` to have the same name.

Type attributes:

| Attribute | Meaning |
|---|---|
| `identities(I, …)` | The type also implements `Quiddity<I>` for each listed `I`. |
| `domain = "L"` | The type's digests are in the domain `L` (4.2). `L` MUST be non-empty. |
| `transparent` | See 6.3. |
| `rename_all = "c"` | On a struct, each field's key is its name in the case convention `c`; on an enum, each variant's name is. The conventions and their rules are serde's: `lowercase`, `UPPERCASE`, `PascalCase`, `camelCase`, `snake_case`, `SCREAMING_SNAKE_CASE`, `kebab-case` and `SCREAMING-KEBAB-CASE`, applied to field names as snake_case and to variant names as PascalCase. A field's or variant's `rename` takes precedence. It does not apply to the fields of struct variants. |
| `encode_as = map \| list` | Structs with named fields (6.3). `encode_as = set` applies to fields only. |
| `encode_as = string \| int` | Enums (6.4). |
| `tag = "t"` | Enums: internally tagged, or with `content` adjacently tagged (6.4). |
| `content = "c"` | Enums: adjacently tagged, with `tag` (6.4). |
| `untagged` | Enums: untagged (6.4). |
| `encode_as = display` | The value is the string produced by the type's `Display` impl. |
| `encode_as = serde` | The value is produced from the type's `Serialize` impl (6.7). Requires the `serde` feature. |

Field attributes:

| Attribute | Meaning |
|---|---|
| `skip` | The field does not participate. |
| `rename = "k"` | The field's key is `k`. |
| `identity = J` | The field's value is encoded under the field type's identity `J`. Without it, a field's value is encoded under the field type's primary identity, whatever identity the enclosing type is being encoded under. |
| `with = path` | The field's value is encoded by calling `path(&field, &mut Encoder)`. |
| `omit_none` | When the field is null, the field's entry is omitted: `None`, `()`, a `serde_json::Value` that is `Null`, or such a value behind a reference or smart pointer. `Some(x)` is not omitted, even when `x` is null. The field's type MUST be one of these; otherwise compile error. |
| `omit_empty` | When the field's value is an empty string, list, set or map, the field's entry is omitted; for an `Option<T>`, when it is `Some` of such a value. The field's type MUST be one that encodes as a string, list, set or map, or an `Option` of one; otherwise compile error. |
| `omit_if = path` | When `path(&field)` returns `true`, the field's entry is omitted. |
| `quantize = q` | The field's value is quantized to a multiple of `q` (5.6). The field's type MUST be an integer type, `Duration` or `SystemTime` (in nanoseconds), or an `Option` of one; otherwise compile error. |
| `decimals = d` | The field's value is quantized to a multiple of `10^−d` (5.6), `d` from −300 to 300. The field's type MUST be `f32`, `f64` or an `Option` of one; otherwise compile error. An `f32` stands for the shortest decimal that rounds to it as a binary32. |
| `encode_as = set` | The field's value is a set of the items of the sequence it holds, each encoded under its primary identity. The field's type MUST be one whose shared reference iterates over references to items (`Vec<T>`, `[T]`, `[T; N]`, `VecDeque<T>`, …); otherwise compile error. |
| `encode_as = display` | As the type attribute, for the field's value. |
| `encode_as = serde` | As the type attribute, for the field's value. |

Variant attributes: `rename = "name"` only.

**Omission.** An omitted field contributes no entry to the map, and is
not counted. A field is omitted when its `omit_none`, `omit_empty` or
`omit_if` condition holds under the identity being encoded. Each condition
is evaluated at most once per encoding. `omit_none`, `omit_empty` and
`omit_if` on a struct encoded as a list are compile errors.

**Applicability.** `encode_as = map | list` on anything but a struct with
named fields, and `encode_as = string | int` on anything but an enum, are
compile errors. So are: `transparent` on an enum; `rename` on a tuple
field; more than one of `with`, `encode_as` and `identity` on a field under
the same identity; a scope list naming an identity not declared in
`identities(…)`; and an attribute given twice for the same identity.

### 6.6 Optional features

| Feature | Types | Data-model value |
|---|---|---|
| `serde_json` | `Value`, `Map`, `Number` | see below |
| `indexmap` | `IndexMap<K, V, S>`, `IndexSet<T, S>` | map, set |
| `uuid` | `Uuid` | string: hyphenated, lowercase, e.g. `67e55044-10b1-426f-9247-bb680e5fe0c8` |
| `bytes` | `Bytes`, `BytesMut` | string: the bytes |
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

`Value` follows the JSON mapping of 2.7. A `Number` holding an `i64` or
`u64` is that integer. A `Number` holding an `f64` is normalized by 2.2.
The `f64` is the one serde_json's parser produced; without serde_json's
`float_roundtrip` feature, that parser is not always correctly rounded, so
a `Value` parsed from a JSON text may differ in the last bit from the value
2.7 gives the text.

With serde_json's `arbitrary_precision` feature, a `Number` holds its
decimal text. Such a `Number` MUST have the value it would have had without
the feature, so that a digest does not depend on a feature that any crate
in the dependency graph can enable: if its text is an integer within the
range of `i64` or `u64`, it is that integer; otherwise it is converted to
the nearest binary64 value (ties to even) and normalized by 2.2. A decimal
value that binary64 cannot represent exactly is therefore rounded, as JCS
(RFC 8785) and I-JSON (RFC 7493) also require.

### 6.7 The `serde` feature

`encode_as = serde` and `quiddity::serde::digest(&value)` encode a value
through its `Serialize` impl, using a serializer that maps serde's data
model as follows. The serializer reports `is_human_readable() == true`.

| serde | Data-model value |
|---|---|
| `bool` | boolean |
| `i8` … `i128`, `u8` … `u128` | integer |
| `f32`, `f64` | normalized by 2.2 |
| `char`, `str` | string: the text's UTF-8 bytes |
| `bytes` | string: the bytes |
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

### 6.8 Exported identities

*Implemented with the `identity` feature.*

`Quiddity<I>` has a provided method, `construct_identity()`, that returns
the identity (section 5) describing the type's encoding under `I`,
or an error. The derive implements it, and so does each implementation in
6.2 and 6.6. Its name says that it builds a structure, which may be costly,
rather than returning one. The default implementation returns an error.

The exported identity is meant for the type's JSON form as serde writes
it. For a value whose serde JSON form has the same field and variant names
as its identity's keys and labels (for example, because the type's serde
and quiddity attributes use the same `rename_all`),
`value.quiddity_as::<I>()` equals the digest of its JSON form under the
exported identity.

| Rust type | Kind |
|---|---|
| `bool` | `"bool"` |
| integers and their `NonZero` forms | `"integer"` |
| `f32`, `f64` | `"number"` |
| `char`, `str`, `String` and the string types of 6.2, `Url`, digests | `"string"` |
| `Uuid` | `"uuid"` |
| `()` | `"null"` |
| `Option<T>` | `{"union": ["null", T]}`, flattened (5.7) |
| `Vec<T>`, slices, arrays, `VecDeque<T>`, `LinkedList<T>` | `{"list": T}` |
| a field with `encode_as = set` | `{"set": T}` |
| tuples | `{"tuple": [T1, …, Tn]}` |
| `BTreeSet<T>`, `HashSet<T>`, `IndexSet<T>` | `{"set": T}` |
| `BTreeMap<K, V>`, `HashMap<K, V>`, `IndexMap<K, V>` | `{"map": {"key": K, "value": V}}` |
| pointers, `Wrapping`, `Saturating`, `Reverse`, `Cell`, atomics | the kind of the type they hold |
| `Duration` | `{"duration": {}}` |
| `SystemTime` | `{"instant": {}}` |
| `Bytes`, `BytesMut`, `CStr`, `CString` | `{"bytes": {}}` |
| `Result<T, E>`, `Bound<T>` | tagged unions with the variants `Ok` and `Err`, and `Included`, `Excluded` and `Unbounded` |
| the range types | objects with the fields `start` and `end`, as they have |
| IP and socket addresses | `"string"` |
| `Path`, `PathBuf`, `OsStr`, `OsString` | `"path"` |
| `serde_json::Value` | `"any"` |
| `serde_json::Map` | `{"map": {"key": "string", "value": "any"}}` |
| `serde_json::Number` | `"number"` |
| a derived struct or enum | `{"ref": name}`, with the type's kind under the field's identity added to `definitions` |

A field with `quantize` or `decimals` exports with the same member. A field
with `omit_none` exports with the condition `"none"`. A field with
`omit_empty` exports as `{"union": ["null", k]}` with the conditions
`"none"` and `"empty"`: serde leaves such a field out when told to skip
empty values (`skip_serializing_if`), and a missing member reads as null
(5.6), which is then omitted as the empty value would have been.

A derived struct with named fields exports as an object kind, with
`order` if it is encoded as a list; a newtype or transparent struct as the
kind of its field; a tuple struct as a tuple; a unit struct as `"null"`; an
enum as a tagged union in its style, or an untagged enum as a union. A
definition's name is the type's name, followed by `/` and the identity's
name if that is not `Primary`; types with the same name are told apart by
their module paths.

`construct_identity()` returns an error if the identity uses `with`,
`omit_if = path`, `encode_as = display` or `encode_as = serde`, which have
no JSON form; if it reaches a type whose serde JSON form no kind
describes (`Infallible`); if it reaches a type whose
implementation does not provide `construct_identity()`; or if the identity
it would return is not valid (5.9), as when an untagged enum has two
variants that a union cannot tell apart.

## 7. Conformance

An implementation conforms to sections 2 to 4 if, for every value, it
produces the encoding of section 3 and the digests of section 4. An
implementation that reads identities also conforms to section 5 if,
for every identity and source value, it gives the data-model value, or the
error, that section 5 specifies.

The normative test vectors are in `test-vectors/quiddity-v1.json`. The file
is generated by `test-vectors/generate.py`, using the Python implementation
of sections 2 to 4 in `test-vectors/qref.py` and of section 5 in
`test-vectors/qident.py`; the generator's header describes the typed JSON
notation for values. The file has these sections:

- `values`: a value, its encoding `E(v)` in hex, and its structural digest
  in each text form (4.4). Floats are given as binary64 bits before
  normalization, so they also test 2.2. Text that contains surrogates is
  given as a list of code points, since JSON cannot hold it, so it also
  tests 2.3.
- `domains`: a value, a domain label, and the digest in that domain in each
  text form.
- `json`: a JSON text and its digest under the mapping of 2.7. Texts with
  the same digest are equal values; several are chosen to test Appendix B.
- `identities`: identities, each with its digest (5.10) and source
  values, given in their JSON forms. Each source has its encoding and digest under the
  identity, or `error: true` if applying the identity MUST fail.
- `invalid_identities`: identities that MUST be rejected (5.9).
- `same_identity`: pairs of identities with the same digest (5.10).
- `identity_of_identities`: Appendix C's identity and its digest under
  itself.
- `text_forms`: strings to be parsed as digest text, whether each is valid,
  and whether accepting or rejecting it is required (`MUST`) or recommended
  (`SHOULD`).

Where the vectors and the prose of sections 2 to 5 disagree, the prose is
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
| bytes `0102` | `0102 0000000000000002 20` | `cee203491d38080c54e55b59fc6328def04b53c438cd9075d71e362572b66409` |
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

## Appendix B. Relation to RFC 8785 (informative)

The JSON Canonicalization Scheme (JCS) [RFC 8785] writes a JSON value as
exactly one JSON text, which can then be hashed. This specification
instead encodes values of its own data model (section 2), which includes
JSON values (2.7).

**Agreement on JSON values.** For JSON texts that are I-JSON [RFC 7493]
(valid Unicode, no duplicate names) and whose integers lie from
−(2^53 − 1) to 2^53 − 1, two texts have the same quiddity digest if and only
if they have the same JCS form. Both treat a number as its binary64 value,
so `1`, `1.0` and `1e0` are equal and `-0` equals `0`. Both treat a string
as its exact code points, without Unicode normalization. Both treat object
members as unordered and array items as ordered. Outside that range,
quiddity is finer: it keeps integers up to 2^64 − 1 exact (2.7), where JCS
rounds them to binary64.

**Differences.**

| | RFC 8785 | This specification |
|---|---|---|
| Integers beyond 2^53 | Rounded to binary64; the RFC suggests strings for them | Exact, at any size (3.4) |
| NaN, ±∞ | An error | Values; every NaN is the same value (2.2) |
| Lone surrogates | An error | Encoded as WTF-8 (2.3) |
| Binary data, sets, non-string keys | Not representable | A string of bytes (2.3), a set, keys of any kind |
| Duplicate keys | An error | Allowed; maps and sets are multisets (2.1) |
| Order of keys | By UTF-16 code units | By encoding (2.5); the orders differ between characters U+E000 to U+FFFF and characters above U+FFFF |
| Framing | JSON text, with escaping | A length or count and a tag after each value, without escaping (3.1) |
| Floats | ECMAScript's shortest round-trip decimal text | The 8 bytes of binary64 (3.4) |
| Hash input | Not specified | Prefixed and domain-separated (4.1, 4.2) |

The differences follow from this specification's purposes. Every value of
the data model can be encoded, so encoding never fails. Values from
languages other than JSON, such as sets, 64-bit integers and binary data,
are encoded exactly. An encoding can be written straight into a hash
function, without producing text and without formatting numbers as
decimals.

## Appendix C. The identity of identities

The identity of identities (5.3), used for the digest of an identity
(5.10). It is itself an identity, and its digest is computed under itself.

```json
{
  "domain": "quiddity.identity",
  "root": {"ref": "Identity"},
  "definitions": {
    "Identity": {"object": {"fields": {
      "domain": {"kind": {"union": ["null", "string"]}, "omit": ["none"]},
      "root": {"kind": {"ref": "Kind"}},
      "definitions": {
        "kind": {"union": ["null", {"map": {"key": "string", "value": {"ref": "Kind"}}}]},
        "omit": ["none", "empty"]
      }
    }}},
    "Kind": {"tagged": {"variants": {
      "null": {}, "bool": {}, "integer": {}, "number": {}, "string": {},
      "uuid": {}, "path": {}, "any": {},
      "bytes": {"kind": {"object": {"fields": {
        "text": {"kind": {"union": ["null", "string"]}, "omit": ["none"]}
      }}}},
      "instant": {"kind": {"ref": "Time"}},
      "duration": {"kind": {"ref": "Time"}},
      "list": {"kind": {"ref": "Kind"}},
      "set": {"kind": {"ref": "Kind"}},
      "tuple": {"kind": {"list": {"ref": "Kind"}}},
      "map": {"kind": {"object": {"fields": {
        "key": {"kind": {"ref": "Kind"}},
        "value": {"kind": {"ref": "Kind"}}
      }}}},
      "object": {"kind": {"ref": "Object"}},
      "union": {"kind": {"set": {"ref": "Kind"}}},
      "tagged": {"kind": {"ref": "Tagged"}},
      "ref": {"kind": "string"}
    }}},
    "Time": {"object": {"fields": {
      "unit": {"kind": {"union": ["null", "string"]}, "omit": ["none"]}
    }}},
    "Object": {"object": {"fields": {
      "fields": {"kind": {"map": {"key": "string", "value": {"ref": "Field"}}}},
      "unlisted": {"kind": {"union": ["null", "string"]}, "omit": ["none"]},
      "order": {"kind": {"union": ["null", {"list": "string"}]}, "omit": ["none"]}
    }}},
    "Field": {"object": {"fields": {
      "kind": {"kind": {"ref": "Kind"}},
      "rename": {"kind": {"union": ["null", "string"]}, "omit": ["none"]},
      "omit": {"kind": {"union": ["null", {"set": "string"}]}, "omit": ["none", "empty"]},
      "quantize": {"kind": {"union": ["null", "integer"]}, "omit": ["none"]},
      "decimals": {"kind": {"union": ["null", "integer"]}, "omit": ["none"]}
    }}},
    "Tagged": {"object": {"fields": {
      "variants": {"kind": {"map": {"key": "string", "value": {"ref": "Variant"}}}},
      "style": {"kind": {"union": ["null", "string"]}, "omit": ["none"]},
      "tag": {"kind": {"union": ["null", "string"]}, "omit": ["none"]},
      "content": {"kind": {"union": ["null", "string"]}, "omit": ["none"]},
      "encode_as": {"kind": {"union": ["null", "string"]}, "omit": ["none"]}
    }}},
    "Variant": {"object": {"fields": {
      "kind": {"kind": {"union": ["null", {"ref": "Kind"}]}, "omit": ["none"]},
      "rename": {"kind": {"union": ["null", "string"]}, "omit": ["none"]},
      "discriminant": {"kind": {"union": ["null", "integer"]}, "omit": ["none"]}
    }}}
  }
}
```

The Rust crate's `Identity` type derives `Quiddity` with portable
attributes only. The identity it exports (6.8) is not this one member
for member: it describes the members that take a fixed set of strings
(styles, units, omit conditions) as tagged unions of those strings rather
than as `"string"`. Applied to any identity in canonical form, it gives
the same digest as this one does.
