# TypeScript and JavaScript port: design

Started and implemented 2026-10-17, in `typescript/`. This document records
the design of the TypeScript/JavaScript implementation of quiddity, as
`history/PYTHON.md` does for Python. Decisions are numbered TS1, TS2, …;
the questions they answered come after them, and the choices made while
implementing are the provisional decisions at the end.

## Context

- **Types are erased.** TypeScript's types do not exist at run time, so
  nothing can be derived from an `interface` without a compiler transform
  (typia, ts-patch) or decorators with metadata. Both tie a library to the
  user's build configuration, which in TypeScript varies more than in any
  other language here. `emitDecoratorMetadata` records only constructors
  (`Number`, `Array`, `Object`), not element types or unions, and
  decorators are split between the legacy and the standard forms.
- **The ecosystem's answer** (zod, TypeBox, Valibot, Effect Schema): a
  schema is a value at run time, and its static type is inferred from it.
- **Numbers.** `number` is binary64, and `JSON.parse` rounds integers
  beyond 2^53 and keeps the last of repeated member names. `bigint` holds
  integers exactly.
- **Text** is UTF-16, and a string can hold lone surrogates, which spec 2.3
  encodes as WTF-8.
- **Hashing.** WebCrypto's `subtle.digest` is asynchronous; `node:crypto` is
  not in browsers.
- **Runtimes.** Node 22 is the oldest maintained LTS (Node 20 reached its end
  of life in April 2026). Node 24, installed here, runs `.ts` files by
  stripping types, and has `Temporal` behind `--harmony-temporal`.
- **TypeScript 7.0** (the native compiler) is current; most projects are
  still on 5.x.
- **npm:** the name `quiddity` is free.

## TS1. Scope

1. The core (spec 2–4): digests of JavaScript values, checked against
   `test-vectors/`.
2. Identities (spec 5): built in code with builders, read from and written
   to JSON, changed, applied to any value, and digested.
3. Types: builders infer the TypeScript types of the values they describe,
   and `objectFor<T>` checks an object kind against an existing type.
4. A class's identity as a static member, `static [q.identity] = …`.

Not in scope yet: code generation from `.ts` types, converters from zod,
TypeBox or JSON Schema, and the schema reconciler discussed alongside this
port.

## TS2. Package

- One package, `quiddity`, for TypeScript and JavaScript: written in
  TypeScript, published as JavaScript with `.d.ts` declarations. From
  JavaScript everything works; TypeScript adds the checks (Q1).
- ESM only, `"type": "module"`, one `exports` entry. ES2022, using only
  standard ECMAScript: `tsconfig.json` has `lib: ["ES2022"]` and no
  `types`, so the compiler rejects Node and DOM APIs in `src/`.
- `engines.node >= 22`. No dependencies. SHA-256 is written in the package
  (TS6).
- Erasable syntax only (`erasableSyntaxOnly`), so Node can run the sources
  and tests by stripping types.
- Built with TypeScript 7.0.2; the packed declarations are also checked by
  TypeScript 5.6 in a consumer project, with `skipLibCheck` off.
- Tests with `node:test`, on the built `dist/`, type-checked by
  `test/tsconfig.json`; `@ts-expect-error` lines test what the compiler
  rejects.

## TS3. JavaScript values in the data model

| JavaScript | Data model | Note |
|---|---|---|
| `null` | null | |
| `undefined` | error; a member whose value is `undefined` is absent | As `JSON.stringify`, and as TypeScript's optional members. |
| `boolean` | boolean | |
| `number` | integer if it has an integer value, else float (spec 2.2) | `-0` is 0; NaN and the infinities are floats. |
| `bigint` | integer | |
| `string` | string, WTF-8 (spec 2.3) | |
| `Uint8Array` (and `Buffer`), `ArrayBuffer`, `DataView` | string, the bytes | Other typed arrays are errors: their bytes depend on byte order. |
| array | list | |
| `Set`, `ValueSet` | set | `ValueSet` can hold equal members. |
| plain object | map with string keys | Own enumerable members, as `JSON.stringify` reads them. |
| `Map`, `ValueMap` | map, keys keep their types | Keys that are the same data-model value (`1`, `1n`) are an error (as Python's PP7). |
| `Date`, `Instant`, `Temporal.Instant`, `Temporal.ZonedDateTime` | instant | An invalid `Date` is an error. `Instant` holds nanoseconds, which a `Date` cannot. |
| `Duration`, `Temporal.Duration` | duration | Weeks are 7 days, days 86 400 seconds, months 2 629 746 seconds (the mean Gregorian month), years 12 months. |
| `Temporal.PlainDate`, `PlainTime`, `PlainYearMonth`, `PlainMonthDay` | string, their ISO text | As Python's `date` and `time`. |
| `Temporal.PlainDateTime` | error | No time zone (spec 2.6), as Python's naive `datetime` (Q4 there). |
| `Digest`, `URL` | string, the text form | |
| an instance of a class with a static `[q.identity]` | under that identity | In its domain at the top level only. |
| any other object | error, naming its type and where | |

Temporal objects are recognized by their `Symbol.toStringTag`, without
referring to `Temporal`, which the ES2022 library does not declare and
most runtimes do not yet have.

## TS4. Core API

- `q.digest(value, identity?)`, `q.encoding(value, identity?)`,
  `q.digestJson(text, identity?)`.
- `Digest`: immutable; `String(d)` and `d.toJSON()` are the text form,
  `Digest.parse(text)` (a `SyntaxError` if not one), `d.bytes`, `d.bits`,
  `d.mini()`, `d.micro()`, `d.inDomain(label)`, `d.equals(other)`.
- `QuiddityError` with `path` and `detail`; `InvalidIdentity` extends it.
- `q.parseJson(text)`: spec 2.7's numbers (a `bigint` beyond 2^53, up to
  64 bits), a repeated member name an error.
- `ValueSet`, `ValueMap`, `Instant`, `Duration`: data-model values that
  JavaScript has no type for.

## TS5. Identities and builders

- `Identity`: `fromJson(text | object)`, `toJson()`, `toJsonText()`,
  `domain`, `name`, `digest`, `encoding`, `read`, `equals`, `named`,
  `withDomain`, `withField`, `withoutField`, as Python's (PY6).
- Builders for every kind of spec 5.4, named as the kinds are, with
  `q.null()` exported under that name; field options as methods
  (`.rename`, `.omit`, `.quantize`, `.decimals`); `kind.named(name)`,
  `kind.inDomain(domain)`; `q.ref(name)` for recursion; `q.variant` for a
  tagged variant's options.
- `Kind<Natural, Accepted>`: the type of the values a kind describes, and
  of those it reads, which is wider where it converts (`integer` describes
  `number | bigint` and reads `"5"`). `q.Infer<K>` and `q.Accepts<K>`.
  `Accepted` is contravariant (a phantom `(value: Accepted) => void`
  member), so a kind fits a field of type `V` when it reads every `V`.
- `q.objectFor<T>(fields)`: every member of `T` that is not a method needs
  a field whose kind reads its type (with `undefined` for an optional
  member), or `q.skip()`; a field `T` does not have is an excess property.
- `static [q.identity]` on a class: an `Identity` or a kind.

## TS6. SHA-256

Written in the package (`src/sha256.ts`), synchronous. WebCrypto would make
every digest a promise, and `node:crypto` is not in browsers. No secret is
ever hashed (spec 4.5), so a constant-time implementation is not needed,
and the test vectors check the output. A dependency (`@noble/hashes`) was
the alternative; the package would no longer be dependency-free, as the
Python package is.

## Questions, resolved 2026-10-17

- **Q1. How much TypeScript:** one package, TypeScript-first in its
  declarations but plain JavaScript at run time; no compiler plugins,
  transforms or decorators, and no required `tsconfig` flag beyond
  `strict` (without `strictNullChecks`, an optional member's `undefined`
  is not checked). TypeScript's types are a convenience, never a guarantee
  about values: an identity checks every value it reads.
- **Q2. A static API:** a checked builder (`objectFor<T>`) in the package,
  which is the compile-time coupling a derive gives (adding a member to `T`
  breaks the build until its field is decided), with field names written
  once more. Code generation from types (a CLI reading `.ts` source and
  JSDoc tags) is the closer analogue of a derive, deferred until wanted;
  decorators were rejected (see Context).
- **Q3. Examples** use neutral subjects (books, shelves) and show the
  import that defines `q`.

## Provisional decisions (implementation, 2026-10-17)

- **TP1. `objectFor<T>(fields)` takes its fields directly.** The design
  sketched `objectFor<T>()(fields)`, curried because TypeScript cannot take
  one type argument and infer another. It infers none: the fields' type is
  `FieldsFor<T>`, and an object literal's excess properties are checked
  without a second type parameter.
- **TP2. `.omit("empty")` does not make a field optional.** Python's
  `OmitEmpty` marker does (PP4). Here the field's kind says what it reads,
  as in the JSON form: `q.optional(q.set(q.string())).omit("none",
  "empty")`.
- **TP3. Under an identity, a class instance is read as its own
  enumerable properties**, whether or not its class has a static identity,
  so the identity applied is the one given. Without one (`q.digest(x)`),
  its class's static identity applies, and a class without one is an
  error, rather than its own properties: a `Decimal` or a `Temporal` object
  from a library would otherwise digest as its internals.
- **TP4. A fresh object literal with a member the kind does not list is a
  type error at `kind.digest({...})`**, by TypeScript's excess-property
  check, though at run time the member is left out. Variables of wider
  types are accepted. Kept: it catches misspelled optional members.
- **TP5. Identities read from text keep each number's text**
  (`RawNumber`), so that `5.0` where an integer is required is rejected
  (spec 5.9). An identity given as an object, from `JSON.parse`, cannot
  tell `5.0` from `5`, and accepts it.
- **TP6. Integers in an identity's JSON form are numbers, or bigints
  beyond 2^53** (a discriminant); `toJsonText()` writes them, where
  `JSON.stringify` cannot.
- **TP7. Error paths** are written as Python's (PP14):
  `$.root.object.fields.title`.
- **TP8. `Identity` is digested through the static-identity mechanism**:
  `Identity[q.identity]` reads an identity as its JSON form under the
  identity of identities, in the domain `quiddity.identity`, so
  `q.digest(identity)` needs no special case.
- **TP9. `q.tagged`, `q.ref` and `q.any` infer `unknown`.** Variants'
  types could be inferred for each style; not done yet.
- **TP10. The identity of identities** is copied into `src/meta.ts` from the
  spec; a test checks it against `QUIDDITY_SPEC.md`.

### Checked against the Python package

The digests of eleven JavaScript values (`Date`, `Set`, `Map` with integer
keys, bytes, a lone surrogate, nested objects, a duration) equal the Python
package's for the corresponding Python values, and an identity built with
`objectFor` has the same digest as the same identity read from JSON by
Python. The tests hold those digests.

### Not done yet

- Tested on Node 24 only (Node 22 is the floor); not run in browsers, Deno
  or Bun, though `src/` uses only ES2022.
- Code generation from types, converters from zod and JSON Schema.
- Speed: about 4 µs per row of five fields, 4.6 under an identity, on Node
  24 on the development machine (macOS).
