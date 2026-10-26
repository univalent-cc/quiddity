# quiddity

*Quiddity* (n.): the essence of a thing; what makes it *what it is*, as
distinct from its incidental properties.

Quiddity computes stable digests of values based on meaning annotations.
A digest is the same in every language that implements quiddity, on every
platform, and in every release.

The encoding is specified in [`QUIDDITY_SPEC.md`](https://github.com/univalent-cc/quiddity/blob/main/QUIDDITY_SPEC.md). The concepts common to every implementation are explained in the
repository's [README](https://github.com/univalent-cc/quiddity#concepts);
this README covers the TypeScript and JavaScript package.

## Installation

```sh
npm install quiddity
```

The package is ESM with type declarations, and has no dependencies. It uses
only standard ECMAScript (ES2022), so it runs in browsers, Deno and Bun as
well as in Node 22 or later, where it is tested. From JavaScript, everything
works the same; TypeScript adds the checks described below, with `strict`
on.

The examples import the package as `q`:

```ts
import * as q from "quiddity";
```

## Digests of values

```ts
import * as q from "quiddity";

const d = q.digest({ title: "Middlemarch", pages: 880 });
String(d); // "sha256:60f3f09b1b02dac0f2c84302c8d450b78fa1ac6f195961467247717dd7b44945"
```

Values are read as their JSON forms would be, so two values that mean the
same thing have the same digest: members in any order, `1` and `1.0`, a
member whose value is `undefined` and no member at all. Where the data
model says more than JSON, it is kept:

| JavaScript | Data model |
|---|---|
| `null` | null |
| `boolean` | boolean |
| `number` | an integer if it has an integer value, else a float |
| `bigint` | integer, exact at any size |
| `string` | string: its text, lone surrogates included |
| `Uint8Array`, `ArrayBuffer`, `DataView` | string: its bytes |
| array | list |
| `Set`, `ValueSet` | set: order does not count |
| plain object | map with string keys |
| `Map`, `ValueMap` | map: keys keep their types, so `1` and `"1"` differ |
| `Date`, `Instant`, `Temporal.Instant`, `Temporal.ZonedDateTime` | instant: integer nanoseconds since 1970, the same in any time zone |
| `Duration`, `Temporal.Duration` | duration: integer nanoseconds, with a month of 2 629 746 seconds, the mean Gregorian month |
| `Temporal.PlainDate`, `PlainTime` | string, ISO 8601 |
| `Digest`, `URL` | string: its text |
| an instance of a class with a static identity | as its identity reads it |

Anything else is an error that says where:

```ts
import * as q from "quiddity";

class Shelf {
  label = "B4";
}
q.digest({ shelves: [new Shelf()] });
// QuiddityError: a Shelf has no data-model value; give its class a static
// [quiddity.identity] (at $.shelves[0])
```

`q.digest` throws `QuiddityError`, with `path` and `detail`. Other values
it rejects: `undefined` outside an object, an invalid `Date`, a
`Temporal.PlainDateTime` (it means a different instant in each time zone),
typed arrays other than `Uint8Array` (their bytes depend on the platform),
and a value that contains itself.

A `Digest` prints as `sha256:` and 64 hex digits (and `JSON.stringify`
writes that text), and is read back with `q.Digest.parse`.
`digest.micro()` gives a 64-bit form, for an ETag, for example, and
`digest.mini()` a 128-bit one. `digest.inDomain(label)` places a digest in a
domain. `q.encoding(value)` gives the bytes a digest hashes.

### JSON text

`JSON.parse` reads every number as a binary64 value, so integers beyond 2^53
lose their value. `q.parseJson` reads JSON text as the spec does: an integer
without a fraction or exponent, up to 64 bits, is exact (a `bigint` beyond
2^53), and a repeated member name is an error. `q.digestJson(text)` digests
JSON text that way.

```ts
import * as q from "quiddity";

q.parseJson('{"isbn": 9780141439549, "copies": 18446744073709551615}');
// { isbn: 9780141439549, copies: 18446744073709551615n }
```

## Identities

An identity says which parts of a value are digested, and how each is read.
It is built in code from kinds, or read from its JSON form, which every
implementation of quiddity reads and writes.

```ts
import * as q from "quiddity";

const Book = q.object({
  title: q.string(),
  pages: q.integer(),
  published: q.instant(),
  genres: q.optional(q.set(q.string())).omit("none", "empty"),
});

const a = Book.digest({ title: "Moby Dick", pages: 635, published: new Date("1851-10-18T00:00:00Z") });
const b = Book.digest({ title: "Moby Dick", pages: "635", published: "1851-10-18T01:00:00+01:00", genres: [] });
a.equals(b); // true
```

Members the identity does not list are left out. Values written in other
forms are converted: the text `"635"` to an integer, RFC 3339 text to an
instant. A value of the wrong shape is an error.

The kinds are those of the spec (5.4): `q.null()`, `q.bool()`,
`q.integer()`, `q.number()`, `q.string()`, `q.uuid()`, `q.path()`,
`q.bytes()`, `q.instant()`, `q.duration()`, `q.any()`, `q.list(k)`,
`q.set(k)`, `q.tuple(k, …)`, `q.map(key, value)`, `q.object({…})`,
`q.union(k, …)`, `q.tagged({…})` and `q.ref(name)`. `q.optional(k)` is
`q.union(q.null(), k)`, which also reads a missing member. A field takes
the options of spec 5.6 as methods: `.rename(key)`, `.omit("none",
"empty")`, `.quantize(step)` and `.decimals(places)`. Times and durations
are best quantized; see [Times and durations](https://github.com/univalent-cc/quiddity#times-and-durations).

`kind.inDomain("example.book")` is the identity of the kind's values in a
domain, an `Identity`; `kind.named("Book")` makes the kind a definition,
which an identity holds once and refers to by name, and which
`q.ref("Book")` refers to from inside itself:

```ts
import * as q from "quiddity";

const Section = q.object({ heading: q.string(), sections: q.list(q.ref("Section")) }).named("Section");
```

### Types from kinds

A kind's TypeScript type is inferred from it:

```ts
import * as q from "quiddity";

const Book = q.object({ title: q.string(), pages: q.integer(), genres: q.optional(q.set(q.string())) });
type Book = q.Infer<typeof Book>;
// { title: string; pages: number | bigint; genres?: Set<string> | null }
```

`q.Infer` is the type of the values a kind describes. A kind also reads
more than that where it converts, and `kind.digest(value)` takes those
values too (`q.Accepts<typeof Book>`): `pages: "635"` is accepted.

### Identities checked against a type

When the type comes first, from a hand-written interface or from
generated code, `q.objectFor<T>` builds an object kind that the compiler
checks against it:

```ts
import * as q from "quiddity";

interface Book {
  title: string;
  pages: number;
  genres?: string[];
  lastOpened: Date; // when you last read it: not part of the book
}

const BookIdentity = q.objectFor<Book>({
  title: q.string(),
  pages: q.integer(),
  genres: q.optional(q.set(q.string())).omit("none", "empty"),
  lastOpened: q.skip(),
});
```

Two copies of the same book have the same digest, whenever each one was
last opened. The compiler makes sure the identity and the type agree:

| You do this | The compiler says |
|---|---|
| Add `author: string` to `Book`, don't touch the identity | Property 'author' is missing in type … but required in type 'FieldsFor<Book>' |
| Write `titel: q.string()` | 'titel' does not exist in type 'FieldsFor<Book>'. Did you mean to write 'title'? |
| Write `pages: q.string()` | Type 'Kind<string, string>' is not assignable to type 'FieldFor<number>' |
| Write `genres: q.set(q.string())`, without `q.optional` | Type 'Kind<Set<string>, …>' is not assignable to type 'FieldFor<string[] \| undefined>' |

Every member of `T` that is not a method needs a field, or `q.skip()`. A
kind that converts is allowed where it reads the member's type:
`q.integer()` for a `string` member, whose text it reads as an integer.

TypeScript's types are not checked at run time, and data from
`JSON.parse(text) as Book` may not be a `Book`. An identity always checks
the values it reads, whatever their type says.

### Classes

A class can carry its identity, which `q.digest` uses for its instances,
in its domain:

```ts
import * as q from "quiddity";

class Book {
  static [q.identity] = q.object({ title: q.string(), pages: q.integer() }).inDomain("example.book");

  title: string;
  pages: number;
  lastOpened = new Date();

  constructor(title: string, pages: number) {
    this.title = title;
    this.pages = pages;
  }
}

q.digest(new Book("Moby Dick", 635)); // under its identity, in example.book
```

A domain applies only at the top level: nested in another value, a
`Book` is digested under its identity, without the domain (spec 4.2).
Under another identity, an instance is read as its own enumerable
properties, as `JSON.stringify` reads it.

### Identities as data

```ts
import * as q from "quiddity";

const book = q.Identity.fromJson(`{
  "domain": "example.book",
  "root": {"object": {"fields": {
    "title": {"kind": "string"},
    "pages": {"kind": "integer"}
  }}}
}`);

q.digestJson('{"pages": 880, "title": "Middlemarch", "lastOpened": "2026-10-17"}', book);
```

`Identity.fromJson` reads JSON text strictly (a repeated member name, or
`5.0` where an integer is required, is an error), or an object decoded from
it. `identity.toJson()` gives the canonical form, and
`identity.toJsonText()` its text, ready for another language.
`q.digest(identity)` is the identity's own digest (spec 5.10), the same in
every language: an identity built in code and the same identity read from
JSON have the same digest.

An identity is immutable and always valid; methods that change it return
a new one, and throw `InvalidIdentity` if the result would not be valid:
`named(name)`, `withDomain(domain)`, `withField(name, field)` and
`withoutField(name)`. `identity.read(value)` gives the data-model value
under it, and `identity.encoding(value)` its bytes.

## Development

```sh
npm install
npm test               # builds, type-checks the tests, and runs them
npm run test:temporal  # the Temporal tests, on Node 24 with --harmony-temporal
```

The tests include the language-neutral vectors in `../test-vectors/`.
