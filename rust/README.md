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

A conceptual overview of quiddity is explained in the repository's
[README](https://github.com/univalent-cc/quiddity#concepts).  That information is
common to every implementation, regardless of language.  This README covers the
Rust crate.

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

```rust
# use quiddity::{Quiddity, QuiddityExt};
# #[derive(Quiddity)]
# struct Task { prompt: String }
# let task = Task { prompt: "Summarize the report".into() };
use http::{HeaderMap, StatusCode, header::IF_MATCH};
use quiddity::Digest;

/// Accepts a change only if the client saw the current version.
fn check_if_match(headers: &HeaderMap, current: Digest) -> Result<(), StatusCode> {
    let claimed: Option<Digest> = headers
        .get(IF_MATCH)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.trim_matches('"').parse().ok());
    match claimed {
        Some(claimed) if claimed == current => Ok(()),
        _ => Err(StatusCode::PRECONDITION_FAILED),
    }
}
# let mut headers = HeaderMap::new();
# headers.insert(IF_MATCH, format!("\"{}\"", task.quiddity()).parse().unwrap());
# assert!(check_if_match(&headers, task.quiddity()).is_ok());
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

Two different kinds of values with the same fields will have the same digest
by default.

When this is not desirable, for example when a digest is signed or used as an
identfier, use a domain. A domain is a label, hashed into the digest, so types
in different domains never share digests.  Set the domain on a type using the
`#[quiddity(domain = "…")]` macro attribute.

The [`Digest::in_domain`] method can also be used to encode a domain into an existing
digest that was computed over a bare structure (no existing domain).

See the [Domains discussion in the repository's README](https://github.com/univalent-cc/quiddity#domains) for more.

## Quantization

A field's value can be rounded down to a grid before it is digested, so
that small differences, such as a float's last digits or a timestamp's
milliseconds, don't change the digest. See the repository's
[README](https://github.com/univalent-cc/quiddity#quantization) for more.

```rust
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use quiddity::{Quiddity, QuiddityExt};

#[derive(Quiddity)]
struct Reading {
    #[quiddity(decimals = 2)]
    price: f64,
    #[quiddity(quantize = 1_000_000_000)] // to the second
    at: SystemTime,
}

let noon = UNIX_EPOCH + Duration::from_secs(1_704_110_400);
let a = Reading { price: 12.989, at: noon + Duration::from_millis(750) };
let b = Reading { price: 12.98, at: noon };
assert_eq!(a.quiddity(), b.quiddity());
```

Values are rounded down, so values on either side of a band's edge have
different digests, however close they are.

## Derive macro attributes

### On a type

| Attribute | Meaning |
|---|---|
| `domain = "…"` | A label hashed into every digest of this type, so that its digests differ from those of other types with the same fields (see [Domains](#domains)). None by default. |
| `identities(A, B, …)` | The named identities this type implements, in addition to the primary one. Each is a type deriving `IdentityMarker`. |
| `transparent` | The type digests as its single field. Exactly one field must remain, after skips, for every identity it applies to, or it is a compile error. |
| `rename_all = "…"` | Rename every field (on a struct) or every variant (on an enum) to a case convention, as serde's `rename_all` does: `"lowercase"`, `"UPPERCASE"`, `"PascalCase"`, `"camelCase"`, `"snake_case"`, `"SCREAMING_SNAKE_CASE"`, `"kebab-case"` or `"SCREAMING-KEBAB-CASE"`. A field's or variant's own `rename` takes precedence. Use the same convention as the type's serde attributes so that its digest keys match its JSON keys. |
| `tag = "…"`, `content = "…"`, `untagged` | How an enum's variants are tagged, as serde's attributes of the same names do: `tag` alone for internal tagging (`{"type": "Rect", "w": 1}`), `tag` and `content` for adjacent tagging (`{"t": "Rect", "c": …}`), `untagged` for the variant's value alone. Without them, `{"Rect": …}`, as serde's default. |
| `encode_as = …` | `map` or `list` for a struct with named fields (default = `map`), `string` or `int` for an enum (default = `string`). `display` for any type that implements `Display`, encode the output. `serde` (with the `serde` feature) for any type that implements `Serialize`, encode it through serde; panics if serialization fails (see [Direct from `serde`](#direct-from-serde)). |

### On a field

| Attribute | Meaning |
|---|---|
| `skip` | Exclude the field from the digest. |
| `rename = "…"` | Provide another name that the field or enum variant is hashed under. |
| `identity = A` | Encode this nested value using its identity `A` instead of its primary one. |
| `with = path` | Digest this field with `path(&value, &mut Encoder)`, for types that do not implement `Quiddity` (see [Foreign types](#foreign-types-and-custom-field-encoding)). |
| `encode_as = set` | Digest this sequence field (`Vec`, slice, array, `VecDeque`, …) as a set, so that the order of its items does not affect the digest. Repeated items are kept. |
| `encode_as = display` | Encode this field as its `Display` output, a string. For types that implement `Display` but not `Quiddity`. |
| `encode_as = serde` | With the `serde` feature: encode this field through its `Serialize` impl. Panics if serialization fails (see [Direct from `serde`](#direct-from-serde)). |
| `omit_none` | When the field is null (`None`, `()`, JSON `null`), omit it, as if the key were missing, rather than hashing it as null. Only valid on types that can be null: `Option`, `()`, `serde_json::Value`, and references and smart pointers to them. `Some(None)` is not omitted. |
| `omit_empty` | When the field is an empty string, sequence, set or map, omit it. On an `Option`, `Some` of an empty value is omitted, and `None` is not (combine with `omit_none` to omit both). |
| `omit_if = path` | Omit the field when `path(&value)` returns `true`, for conditions the two above do not cover. The cost of the check is the caller's. |
| `quantize = n` | Round the field's value down to a multiple of `n`, so that smaller differences do not change the digest. For integers, and for `Duration` and `SystemTime` in nanoseconds (`quantize = 1_000_000_000` is to the second), or an `Option` of one. |
| `decimals = d` | Round the field's value down to `d` decimal places (to hundreds with `d = -2`). For `f32` and `f64`, or an `Option` of one. A float is taken as the decimal it is written as, so `2.67` stays `2.67`. |

Omitting a field lets it be added to a type without changing the digests of
existing values, as long as the new field is omitted in them.

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
| `std` | yes | `HashMap`, `HashSet`, `SystemTime`, `Path`, `PathBuf`, `OsStr` and `OsString` | `alloc` |
| `alloc` | yes | `String`, `Vec`, `VecDeque`, `LinkedList`, `Box`, `Rc`, `Arc`, `Cow`, `BTreeMap`, `BTreeSet` and `CString` | |
| `serde_json` | no | `Value`, `Map` and `Number` | `alloc` |
| `serde` | no | `encode_as = serde` and `quiddity::serde::digest`; `Serialize` and `Deserialize` for `Digest`, `Digest128` and `Digest64`, and with `identity` for `Identity` (as its identity document) | `alloc` |
| `indexmap` | no | `IndexMap` and `IndexSet` | `alloc` |
| `uuid` | no | `Uuid` | |
| `bytes` | no | `Bytes` and `BytesMut` | `alloc` |
| `url` | no | `Url` | `std` |
| `identity` | no | [Dynamic identities](#dynamic-identities): identity documents, and digests of `serde_json::Value`s under them | `derive`, `serde_json` |

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
  field is null unless it has `omit_none`.
- `Box`, `Rc`, `Arc` and references are digested as the value they point to.
- `char` is digested as a one-character string.
- `bool` is digested as `true` or `false`.
- `()` is digested as null.
- Text and binary data are both strings. Text is digested as its UTF-8
  bytes, so `"ab"` and the bytes `[0x61, 0x62]` written with
  [`Encoder::bytes`] have the same digest.
- `Duration` is digested as an integer number of nanoseconds, and
  `SystemTime` as the integer number of nanoseconds since the Unix epoch.
- IP and socket addresses are digested as their text form, such as
  `"192.0.2.1"` or `"[2001:db8::1]:80"`.
- `Path`, `PathBuf`, `OsStr` and `OsString` are digested as the platform's
  bytes for the path. A path that is valid UTF-8 has the same digest as the
  same `str`. Paths are not normalized.
- `Result` is digested as `{"Ok": x}` or `{"Err": e}`, and ranges as
  `{"start": a, "end": b}`, the forms serde writes in JSON.
- `Wrapping`, `Saturating`, `Reverse`, `Cell` and the atomic integers are
  digested as the value they hold.

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
JSON. To digest a field as a string of bytes instead, use `with` and
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
  key as a `None` field with `omit_none`.
- Numbers are compared by value: `1`, `1.0` and `1e0` have the same digest,
  which is also the digest of an integer field holding `1`.
- serde_json's `arbitrary_precision` feature does not change digests either:
  a number digests as it would without the feature. A number that is
  neither an integer within `i64` or `u64` nor exactly representable as an
  `f64` is rounded to the nearest `f64`.
  A number is digested as the `f64` that serde_json parsed. serde_json's
  default parser can be off by one in the last bit for some numbers, which
  then digest differently than in other languages, whose parsers are
  exact. serde_json's `float_roundtrip` feature makes its parser exact.
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
  strings of their bytes. Unlike a `Vec<u8>`, they do not have the same
  digest as a JSON array of numbers.
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
bytes are strings rather than arrays of numbers, and integers wider than 64
bits are kept.

A type's `Serialize` impl may give it a different digest from its
`Quiddity` impl. For example, serde writes a `Duration` as
`{"secs": s, "nanos": n}`, not as nanoseconds.

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
struct Project {
    #[quiddity(encode_as = display)]
    homepage: url::Url,
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

[`Encoder`] writes the primitive kinds (`null`, `bool`, `int`, `float`, and
strings with `str` or `bytes`) and the composite ones (`list`, `set`, `map`, `fields`). A manual
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
`arrayvec`, `chrono`, `time`, `jiff`, `rust_decimal`, `bson`, Amazon Ion, CBOR,
MessagePack, TOML or YAML values. Please open an issue and / or a pull request,
if you would like to see an implementation for another type.  Please say how
the type's values should be digested. In the meantime, the `serde` feature
covers most types that implement `Serialize`.

## Dynamic identities

An identity can also be written as data: an identity document, in JSON,
that every implementation of quiddity can read (see the repository's
[README](https://github.com/univalent-cc/quiddity#identity-documents) and
[`QUIDDITY_SPEC.md`] section 5). The `identity` feature loads identity
documents and digests `serde_json::Value`s under them:

```rust ignore
use quiddity::identity::Identity;
use serde_json::json;

let identity = Identity::from_json(&json!({
    "quiddity": 1,
    "root": {"object": {"fields": {
        "prompt": {"kind": "string"},
        "priority": {"kind": "integer"},
        "asked_at": {"kind": {"instant": {}}}
    }}}
}))?;

let a = identity.digest(&json!({
    "prompt": "p", "priority": 1, "asked_at": "2024-01-01T00:00:00Z", "tag": "x"
}))?;
let b = identity.digest(&json!({
    "prompt": "p", "priority": "1", "asked_at": "2024-01-01T01:00:00+01:00"
}))?;
assert_eq!(a, b);
```

Members that the identity does not list, such as `tag`, are skipped.
Values written in other forms are converted: the text `"1"` to an
integer, and RFC 3339 text to an instant. A value of the wrong shape is an
error.

### Building identities at runtime

An [`Identity`] can also be built and changed in code. Kinds nest: a
field's kind can be another identity, which becomes a definition of the
identity it is placed in, under its name. Each method checks what it
builds, and a change that would make an identity invalid fails, changing
nothing.

```rust ignore
use quiddity::identity::{Field, Identity, Kind};

let row = Identity::new(Kind::object([
    ("label", Kind::String),
    ("value", Kind::Number),
])?)?
.named("Row");

let mut question = Identity::new(Kind::object([
    ("brief", Field::new(Kind::String)),
    ("note", Field::optional(Kind::String)),             // null or missing: left out
    ("tags", Field::omit_empty(Kind::set(Kind::String))),
    ("rows", Field::new(Kind::list(row))),
    ("parent", Field::optional(Kind::recursive())),       // a question
])?)?
.named("Question");
question.set_domain("example.question")?;
question.set_field_in("Question", "asked_at", Kind::instant())?;

let document = question.to_json(); // {"quiddity": 1, "root": {"ref": "Question"}, …}
```

The [`identity::Kind`] and [`identity::Field`] documentation has an
example for each kind and method.

An `Identity` has a digest of its own, `identity.quiddity()`, which every
implementation computes the same way.

A type that implements `Quiddity` exports its identity with
`construct_identity()`, ready to be written out and used in another
language:

```rust ignore
let identity = <Question as Quiddity>::construct_identity()?;
let document = identity.to_json();
```

A value's digest then equals the digest of its serde JSON form under the
exported identity, provided the type's serde and quiddity attributes name
fields and variants the same way (for example, with the same
`rename_all`). Export fails for what an identity document cannot express,
such as a `with` function or `encode_as = display`.

## More examples

See the [`examples`](https://github.com/univalent-cc/quiddity/tree/main/rust/examples) directory.

## Alternatives to Quiddity

The repository's
[README](https://github.com/univalent-cc/quiddity#alternatives-to-quiddity)
compares `quiddity` with related libraries and formats, in Rust and other
languages.

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
[`Digest::in_domain`]: https://docs.rs/quiddity/latest/quiddity/struct.Digest.html#method.in_domain
[`IdentityMarker`]: https://docs.rs/quiddity/latest/quiddity/trait.IdentityMarker.html
[`Primary`]: https://docs.rs/quiddity/latest/quiddity/struct.Primary.html
[`Encoder`]: https://docs.rs/quiddity/latest/quiddity/struct.Encoder.html
[`Encoder::bytes`]: https://docs.rs/quiddity/latest/quiddity/struct.Encoder.html#method.bytes
[`quiddity::serde::digest`]: https://docs.rs/quiddity/latest/quiddity/serde/fn.digest.html
[`identity::Kind`]: https://docs.rs/quiddity/latest/quiddity/identity/enum.Kind.html
[`identity::Field`]: https://docs.rs/quiddity/latest/quiddity/identity/struct.Field.html
[`Identity`]: https://docs.rs/quiddity/latest/quiddity/identity/struct.Identity.html
