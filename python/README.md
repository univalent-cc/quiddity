# quiddity-digest

*Quiddity* (n.): the essence of a thing; what makes it *what it is*, as
distinct from its incidental properties.

Quiddity computes stable digests of values based on meaning annotations.
A digest is the same in every language that implements quiddity, on every
platform, and in every release.

The encoding is specified in [`QUIDDITY_SPEC.md`](https://github.com/univalent-cc/quiddity/blob/main/QUIDDITY_SPEC.md). The concepts common to every implementation are explained in the
repository's [README](https://github.com/univalent-cc/quiddity#concepts);
this README covers the Python package.

## Installation

```sh
pip install quiddity-digest
```

The package is imported as `quiddity`. It needs Python 3.11 or later, and
has no dependencies. Support for pydantic models needs pydantic 2, which
the `pydantic` extra installs: `pip install 'quiddity-digest[pydantic]'`.

## Digests of values

```python
>>> import quiddity as q
>>> digest = q.digest({"title": "Moby Dick", "pages": 635})
>>> str(digest)
'sha256:4188827b9812bba3630f092d3cce34db66dcc8992df7afe02eb37d6365347382'

```

Values are read as their JSON forms would be, so two values that mean the
same thing have the same digest: dict keys in any order, `1` and `1.0`, a
tuple and a list. Where the data model says more than JSON, it is kept: a
`datetime` is an instant, the same in any time zone; a `set` is a set,
whose order does not count; bytes are bytes; and dict keys keep their
types. What has no data-model value is an error, which says where:

```python
>>> from datetime import datetime
>>> q.digest({"steps": [1, datetime(2026, 10, 15, 9, 30)]})
Traceback (most recent call last):
...
quiddity.QuiddityError: a naive datetime has no instant: ... (at $.steps[1])

```

A naive `datetime` has no instant: it means a different time on each
machine. `q.digest` raises `quiddity.QuiddityError`, a `ValueError` with
`.path` and `.message`.

A `Digest` prints as `sha256:` and 64 hex digits, and is read back with
`Digest.parse`. `digest.micro()` gives a 64-bit form, for an ETag, for
example, and `digest.mini()` a 128-bit one.

## Declared classes

The fields of a dataclass, an attrs class or a pydantic model are
digested under their names. Options are markers in the field's
`Annotated` type, and options for the class as a whole go in the
`@q.digestible` decorator:

```python
>>> from dataclasses import dataclass
>>> from typing import Annotated
>>> @q.digestible(domain="example.book")
... @dataclass
... class Book:
...     title: str
...     genres: Annotated[list[str], q.AsSet()]
...     catalogued_at: Annotated[datetime, q.Quantize(1_000_000_000)]
...     subtitle: Annotated[str | None, q.OmitNone()] = None
...     last_opened: Annotated[datetime | None, q.Skip()] = None

```

The digest covers every field except `last_opened`, when you last read
the book, which is not part of it. The order of `genres` does not count,
`catalogued_at` is rounded down to the second, and `subtitle` is left out
when it is `None`, so that adding it did not change the digests of
existing books. The domain keeps a book's digests apart from those of any
other type with the same fields.

A class can have more than one identity, for different purposes. A marker
given identity names applies under those identities only:

```python
>>> @q.digestible(identities=["meaning"])
... @dataclass
... class Book:
...     title: str
...     pages: Annotated[int, q.Skip("meaning")]
>>> a, b = Book("Moby Dick", 635), Book("Moby Dick", 720)
>>> q.digest(a) == q.digest(b), q.digest(a, "meaning") == q.digest(b, "meaning")
(False, True)

```

### Markers

| Marker | Meaning |
|---|---|
| `Skip()` | Leave the field out. |
| `Rename("key")` | Digest it under another key. |
| `OmitNone()` | Leave it out when it is `None`, so that a field added later does not change the digests of existing values. |
| `OmitEmpty()` | Leave it out when it is `None`, or an empty string, list, set or dict. |
| `Quantize(n)` | Round an integer, a `datetime` or a `timedelta` down to a multiple of `n`, in nanoseconds for times (`1_000_000_000` is to the second). |
| `Decimals(d)` | Round a number down to `d` decimal places (to hundreds with `-2`), taking a float as the shortest decimal that reads back as it. |
| `AsSet()` | Digest a list as a set: the order of its items does not count. Repeated items are kept. |
| `Flatten()` | Write the members of a nested class or dict into the containing class's map. |
| `Nested("name")` | Digest a nested declared class under its identity `name`. |

Each marker takes identity names after its own arguments:
`Rename("sort", "meaning")` renames the field under `meaning` only. Under
an identity, a marker scoped to it takes precedence over an unscoped one.

`@q.digestible` takes `domain`, `identities` (the names besides
`"primary"`), and `rename_all`, a case convention for every field's key
as serde names them (`"camelCase"`, `"kebab-case"`, …). A dataclass
without the decorator is digested with the defaults.

A class can also digest itself: `__quiddity__(self, identity)` returns the
value to digest in its place.

## Domains

Two kinds of values with the same fields have the same digest by default.
When this is not desirable, for example when a digest is signed or used as
an identifier, use a domain. A domain is a label, hashed into the digest,
so values in different domains never share digests. A class's domain is
set with `@q.digestible(domain=...)`, and `Digest.in_domain(label)` puts
an existing digest in a domain.

## Quantization

A field's value can be rounded down to a grid before it is digested, so
that small differences, such as a float's last digits or a timestamp's
milliseconds, don't change the digest:

```python
>>> from datetime import UTC
>>> @dataclass
... class Reading:
...     price: Annotated[float, q.Decimals(2)]
...     at: Annotated[datetime, q.Quantize(1_000_000_000)]
>>> a = Reading(12.989, datetime(2024, 1, 1, 12, 0, 0, 750_000, tzinfo=UTC))
>>> b = Reading(12.98, datetime(2024, 1, 1, 12, 0, 0, tzinfo=UTC))
>>> q.digest(a) == q.digest(b)
True

```

Values are rounded down, so values on either side of a band's edge have
different digests, however close they are.

## Identities as data

An identity can also be data. It is written as a type, as pydantic's
`TypeAdapter` takes any type, and applies to any value, such as a dict
decoded from JSON:

```python
>>> from typing import TypedDict
>>> class Book(TypedDict):
...     title: str
...     pages: int
...     published: datetime
>>> identity = q.Identity(Book)
>>> a = identity.digest(
...     {"title": "Moby Dick", "pages": 635, "published": "1851-10-18T00:00:00Z", "tag": "x"}
... )
>>> b = identity.digest(
...     {"title": "Moby Dick", "pages": "635", "published": "1851-10-18T01:00:00+01:00"}
... )
>>> a == b
True

```

Members the identity does not list are left out, and values written in
other forms are converted: the text `"635"` to an integer, RFC 3339 text to
an instant. `NotRequired` members may be missing, and `Annotated` markers
say what a type cannot: `Annotated[datetime, q.Unit("ms")]` reads numbers
as milliseconds, and `Annotated[Circle | Square, q.Tagged("type")]` is a
union told apart by its `type` member, as pydantic's discriminated unions
are.

Every implementation of quiddity reads and writes an identity's JSON form:
`identity.to_json()` writes it, and `q.Identity.from_json(text)` reads it.
`q.identity_of(Book)` exports a declared class's identity, for other
languages to apply to its JSON form. An identity is immutable; its
`with_field`, `without_field`, `named` and `with_domain` methods return
changed copies, and `q.digest(identity)` is its own digest.

## pydantic

A pydantic model is digested as its configuration says: under its
fields' aliases, without excluded fields, and with each custom
`field_serializer` applied. Its JSON form, written with
`model_dump_json(by_alias=True)`, has the same digest under its exported
identity. A model with a `model_serializer`, and a `RootModel`, is
digested as it serializes itself.

## Requesting support for other packages

Support for more packages is welcome, for example msgspec `Struct`s,
NumPy scalars, `whenever` and `pendulum` datetimes, BSON `ObjectId`s,
SQLAlchemy and Django models, or marshmallow schemas. Please open an issue and / or a pull request if you
would like to see support for another package's types. Please say how its
values should be digested. In the meantime, a class can digest itself with
a `__quiddity__(self, identity)` method.

## Examples

The [`examples`](https://github.com/univalent-cc/quiddity/tree/main/python/examples)
directory has scripts to run with `python examples/<name>.py`.

## License

MIT.
