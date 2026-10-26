# Python port: design

Started 2026-10-15. This document records the design of the Python
implementation of quiddity, as `history/DESIGN.md` records the
specification and the Rust crate, and `history/ELIXIR.md` the Elixir
packages. Decisions are numbered PY1, PY2, …; the questions they answered
are collected after them. Implemented 2026-10-15 in `python/`; the choices
made while implementing are the provisional decisions at the end.

## Context

- **Python:** 3.11, 3.12 and 3.14 are installed here; the tests pass on
  each. Python 3.10 reaches its end of life
  this month (October 2026); 3.11 in October 2027.
- **Declaring structures.** Unlike Elixir, Python declares a class's fields
  with types that can be read at run time (`typing.get_type_hints`):
  - `dataclasses` (standard library): fields with types, and per-field
    metadata with `field(metadata=...)`;
  - `typing.Annotated` (standard library): metadata attached to a field's
    type, `price: Annotated[float, Decimals(2)]`, which pydantic and
    msgspec use for the same purpose;
  - pydantic 2.14 (the most used third-party library, requiring Python
    3.10+): models with aliases, excluded fields and serializers; most of
    Python's JSON APIs (FastAPI among them) are defined with it, so it is
    Python's serde;
  - attrs (26.1) and msgspec (0.22): similar in shape, less common.
- **JSON in the standard library** (`json`): writes `None`, `bool`, `int`,
  `float` (including `NaN` and `Infinity`, which are not JSON), `str`,
  `list` and `tuple` (as arrays), and `dict` (with `str`, `int`, `float`,
  `bool` and `None` keys, written as strings). It refuses `bytes`, `set`,
  `datetime`, `Decimal`, `UUID` and other classes unless given a `default`.
  So, unlike Elixir's encoders, it decides little about rich types; pydantic's
  JSON mode decides more (`datetime` as ISO 8601, `Decimal` and `UUID` as
  strings, `set` as an array, `Enum` as its value, `bytes` as UTF-8 text).
- **The references** `test-vectors/qref.py` (spec 2–4) and
  `test-vectors/qident.py` (spec 5) are Python already, written for
  clarity, and generate the vectors.
- **PyPI:** the name `quiddity` is taken by an active, unrelated project
  (quiddity 0.3.10, feature recognition for build123d solids, last upload
  2026-10-02). `quiddity-digest`, `pyquiddity`, `quiddity-py` and
  `quiddity-hash` are free (Q1).

## PY1. Scope and order of work

The same layers as Rust and Elixir:

1. The core (spec 2–4): digests of Python values, checked against
   `test-vectors/`.
2. Dynamic identities (spec 5): `Identity`, built in code or read from
   JSON, applied to any value.
3. Declared classes: dataclasses (and attrs classes, and `NamedTuple`s)
   with options in `Annotated` metadata, the counterpart of the derive;
   identities exported from their type hints.
4. pydantic: a model's own configuration (aliases, exclusions) as its
   identity, the counterpart of `serde_attributes` and `ash_quiddity`.

## PY2. Package

- A `python/` directory in the repository, with a `pyproject.toml`, the
  `src/` layout, and a `py.typed` marker (the package is typed).
- Distributed as `quiddity-digest`, imported as `quiddity` (Q1).
- `requires-python = ">=3.11"`, tested on 3.11 through 3.14 (Q2). A pure
  Python package runs unchanged on later versions; the floor is the oldest
  version still supported.
- No dependencies. SHA-256 is `hashlib`. pydantic support is an optional
  extra (`quiddity-digest[pydantic]`), the usual Python packaging for an
  optional integration, imported only when used (where Elixir has a
  separate `ash_quiddity` package, Python's extras serve the same purpose).
- Tests with pytest, reading `../test-vectors/quiddity-v1.json` and
  skipping when it is absent, as the Rust and Elixir tests do.
- `qref.py` and `qident.py` stay where they are, separate from the
  package: the vectors must not come from the code they test (D19).

## PY3. Python values in the data model

The mapping (spec 2), following `qref.py` where it covers a type, and
otherwise what pydantic's JSON mode writes, except where the data model
says more than JSON (sets, instants, durations, bytes, non-string keys),
as in Rust and Elixir:

| Python | Data model | Note |
|---|---|---|
| `None` | null | |
| `bool` | boolean | Checked before `int`, which `bool` subclasses. |
| `int` | integer | Exact, any size. |
| `float` | number, normalized (spec 2.2) | `NaN` and `±inf` are values of the data model, as in Rust. |
| `str` | string, its WTF-8 bytes (spec 2.3) | Lone surrogates, which a `str` can hold, are kept (as `qref.wtf8`). |
| `bytes`, `bytearray`, `memoryview` | string, its bytes | As `qref.py`: text and bytes are one kind (D17). |
| `list`, `tuple` | list | `json` writes tuples as arrays. |
| `set`, `frozenset` | set | |
| `dict`, `Mapping` | map | Keys keep their kinds: `{1: "a"}` has an integer key, as Rust's `HashMap<u32, _>` and Elixir's maps (`json` would make it `"1"`). |
| `datetime` with a time zone | instant, integer nanoseconds (spec 2.6) | |
| naive `datetime` | instant, read as UTC | Spec 2.6 (Q4). |
| `date` | string, ISO 8601 | As pydantic writes it; the data model has no date kind. |
| `time` | duration, integer nanoseconds since midnight | Spec 2.6. Its `tzinfo` does not count. |
| `timedelta` | duration, integer nanoseconds | Exact: days, seconds and microseconds. |
| `Decimal` | string, `str(d)` | As pydantic, msgspec and Django write it; the same text as Elixir's (Q5). |
| `UUID` | string, lowercase hyphenated | |
| `PurePath` | string, `os.fsencode(path)` | The path's bytes: its text when valid UTF-8, its exact bytes otherwise, as Rust's `Path` on Unix (Q6). |
| `Enum` | its value, mapped by this table | As pydantic and `json` (for `IntEnum`, `StrEnum`) write it. |
| `Digest` | string, its text form | Spec 6.2. |
| a declared class (PY5) | through its declaration | |
| anything else | error, naming the type and where | |

## PY4. Core API

Python idioms: exceptions rather than result tuples, keyword arguments,
frozen value types.

- `quiddity.digest(value, identity="primary") -> Digest`, raising
  `quiddity.QuiddityError` (a `ValueError`) with `.path` and `.message`
  for a value it cannot digest. `identity` is the name of one of a declared
  class's identities, or an `Identity`.
- `Digest`: frozen, hashable, comparable; `str(d)` is the text form,
  `d.bytes` the bytes, `Digest.parse(text)` reads the text form (raising
  `ValueError`), `d.mini()` and `d.micro()` the 128- and 64-bit forms,
  `d.in_domain(label)`.
- `quiddity.encoding(value) -> bytes`, the spec's E(v), for tests and
  other implementations (Rust and Elixir keep it internal; Python's
  references use it).

## PY5. Declared classes

Options as `Annotated` metadata on a field's type, the idiom pydantic and
msgspec use, and class-level options in a decorator:

```python
from dataclasses import dataclass
from datetime import datetime
from typing import Annotated

import quiddity as q

@q.digestible(domain="example.task", identities=["meaning"])
@dataclass
class Task:
    prompt: str
    priority: Annotated[int, q.Skip("meaning")]
    tags: Annotated[list[str], q.AsSet()]
    asked_at: Annotated[datetime, q.Quantize(1_000_000_000)]
    note: Annotated[str | None, q.OmitNone()] = None
    submitted_at: Annotated[datetime | None, q.Skip()] = None

q.digest(task)             # every field but submitted_at
q.digest(task, "meaning")  # and not the priority
```

- Markers: `Skip`, `Rename`, `OmitNone`, `OmitEmpty`, `Quantize`,
  `Decimals`, `AsSet`, `Flatten`, `Nested(identity)`. Each takes optional
  identity names to scope it, as Rust's `skip(Meaning)`: `Skip()` applies
  to every identity, `Skip("meaning")` to `meaning` only.
- Markers only, in `Annotated` (Q3): no `field(metadata=...)` spelling.
- The decorator is optional for a plain dataclass: without it, every field
  counts, under its name. It carries what the class as a whole decides:
  `domain`, `identities`, `rename_all`.
- Identities are named by strings, the primary one `"primary"`: Python has
  no marker types to need.
- attrs classes are read the same way (their fields and type hints). A
  `NamedTuple` is a tuple (PP1). A `TypedDict`'s values are dicts, and the
  class itself can be decorated, for its domain and `rename_all` as an
  identity (PP15).
- `quiddity.identity_of(Task, "meaning")` exports the class's identity,
  its kinds read from the type hints, as Rust's `construct_identity()`
  (the mapping is PY7's, applied to types).

## PY6. Dynamic identities

The same operations as Rust's and Elixir's: create, change, load, write,
digest any value, and the identity's own digest.

- `Identity.from_json(text_or_dict)`, `identity.to_json() -> dict`.
- Kinds are Python types, mapped as PY7 does, as pydantic's
  `TypeAdapter(list[int])` takes any type (Q7): `Identity(list[str])`,
  `Identity(dict[str, int])`, `Identity(Task)` for a declared class. A
  `TypedDict` is an object kind without a class (`NotRequired[...]` for an
  optional member). What a type cannot say is an `Annotated` marker, as
  pydantic does: `Annotated[datetime, q.Unit("ms")]`,
  `Annotated[bytes, q.Hex()]`, `Annotated[Shape, q.Tagged(tag="type")]`
  (as pydantic's `Field(discriminator="type")`). Identities in their JSON form are read with
  `Identity.from_json`. `Identity(Task)` is the same as `identity_of(Task)`,
  so one mapping serves export and identities written in code.
- Changes as in Rust: `named`, `with_domain`, `with_field`,
  `without_field`, `with_field_in`, `without_field_in`, returning new
  identities (an `Identity` is immutable and hashable, as `Digest` is).
- `q.digest(identity)` is the identity's own digest (spec 5.10).

## PY7. Types as kinds

For exporting declared classes (PY5) and for kinds written as types (PY6):

| Type | Kind |
|---|---|
| `int` | integer |
| `float` | number |
| `str` | string |
| `bool` | bool |
| `bytes` | bytes |
| `datetime` | instant |
| `timedelta` | duration |
| `UUID` | uuid |
| `PurePath` | path |
| `date`, `time`, `Decimal` | string |
| `list[T]`, `Sequence[T]`, `tuple[T, ...]` | list |
| `tuple[A, B]` | tuple |
| `set[T]`, `frozenset[T]` | set |
| `dict[K, V]`, `Mapping[K, V]` | map |
| `T \| None`, `Optional[T]` | optional |
| other unions | union, if valid; else `any` |
| `Literal["a", "b"]`, `Enum` of strings | string |
| a declared class | its identity, nested; itself: recursive |
| `Any`, unknown | any |

## PY8. pydantic

A model is digested as its configuration says, the counterpart of
`serde_attributes` (D21) and `ash_quiddity`: field aliases (or
`serialization_alias`, or the model's `alias_generator`), excluded fields
(`Field(exclude=True)`), and `Annotated` markers (PY5) mixed in.

Built on the model's fields (`model_fields`, their types and `FieldInfo`),
below serialization (Q8): values keep their types (an instant, a set),
digests do not depend on pydantic's output format, and nothing is
serialized. A field with a `field_serializer` is digested through that
serializer's output, as Rust's `serialize_with` through serde's bridge.
Computed fields count only when included, as Ash calculations.

## Questions, resolved 2026-10-15

- **Q1. The distribution name:** `quiddity-digest`, imported as
  `quiddity`. "Digest" is the spec's word and the `Digest` type's; "hash"
  suggests Python's `__hash__`, which is per-process and not
  cryptographic. `quiddity` itself is taken on PyPI by an unrelated CAD
  library.
- **Q2. The Python version:** `>=3.11`, tested through 3.14. A floor, not
  a target: a pure Python package runs on later versions unchanged, so
  users on 3.12 or later are unaffected; only compiled extensions are
  built per version. Nothing here needs syntax newer than 3.11.
- **Q3. Where options go:** `Annotated` markers only (PEP 593), as pydantic
  (`Field`, constraints), msgspec (`Meta`) and the `annotated-types`
  library use them; they work for dataclasses, attrs, NamedTuples and
  pydantic models alike. `field(metadata=...)`, the older dataclass-only
  idiom (dataclasses-json, marshmallow-dataclass), is not a second
  spelling.
- **Q4. Naive `datetime`:** read as UTC (spec 2.6), as in every
  language, so that the same naive date and time digests alike in all of
  them. It was first an error, because a naive datetime is often an
  accident (`datetime.now()`); that was changed (2026-10-26): whether a
  naive time was meant as UTC is the caller's concern, and a digest must
  only be the same everywhere. It is never `datetime.timestamp()`, which
  takes a naive datetime as the machine's local time.
- **Q5. `Decimal`:** `str(d)`, as pydantic, msgspec (by default) and
  Django's JSON encoder write it. `str(Decimal)` implements the General
  Decimal Arithmetic specification's "to-scientific-string", as Elixir's
  `Decimal.to_string/1` does: the two agree (checked on 13 values, among
  them `1E+10`, `1.2E-7`, `0E-7`, `-0` and `12.50`). Elixir's P5 was changed
  to it, so a decimal digests alike in both.
- **Q6. Paths:** `os.fsencode(path)`, the standard way to a path's bytes:
  its text for a valid UTF-8 path, its exact bytes otherwise, as Rust on
  Unix. pydantic writes `str(path)`, the same for valid UTF-8.
- **Q7. Kinds as types:** Python types map onto kinds wherever possible,
  with `Annotated` markers for the rest and `TypedDict` for objects without
  a class, as pydantic's `TypeAdapter` takes any type (PY6).
- **Q8. The pydantic level:** the model's fields, below serialization,
  with custom serializers followed per field (PY8). Code that hashes
  pydantic models usually hashes `model_dump_json()` (as ETag middleware
  hashes response bodies); libraries that compare objects (deepdiff's
  `DeepHash`, pydantic's own `__hash__` for frozen models) walk fields. The
  serialized form loses types (an instant becomes text, a set a list), and
  depends on pydantic's output format, which changed between v1 and v2
  (`timedelta`, among others); that would change digests. Rust's
  `serde_attributes` and Elixir's `ash_quiddity` build on the same level.

## Provisional decisions (implementation, 2026-10-15)

Taken while implementing `python/`, where the choice was not clear, to be
revisited. Each says what was chosen, the alternatives, and why.

- **PP1. A `NamedTuple` is a tuple**, digested as a list and exported as a
  `tuple` kind, rather than as a declared class (PY5 said "read the same
  way"). `json` and pydantic write a NamedTuple as an array, so its JSON
  form is an array, and its digest should be that form's.
- **PP2. Nested values use their primary identities.** `digest(task,
  "meaning")` reads `task` under `meaning`, and its nested classes under
  `"primary"`, unless a field has `Nested("name")`, as Elixir's
  `identity:` option. A `None` passes through `Nested`.
- **PP3. Hand-written digests:** a class with a `__quiddity__(self,
  identity)` method is digested as the value it returns, and a class
  attribute `__quiddity_domain__` gives its domain, the counterparts of a
  manual `Quiddity` impl and its `DOMAIN`. Dunder methods are Python's
  idiom for such protocols (`__json__`, `__fspath__`). A domain applies
  only at the top level (D4).
- **PP4. `OmitEmpty` means empty or missing**: it also omits `None`, and
  in an identity the field becomes optional (`omit: ["empty", "none"]`),
  as Rust's `Field::omit_empty` and Elixir's export do. Without it, a
  field exported with `OmitEmpty` would reject the `null` that the value
  side digests.
- **PP5. Exported fields are named by their JSON members**: an attribute's
  name, or a pydantic alias, with `rename` for the digest key when a
  `Rename` or `rename_all` changes it. The exported identity then applies to
  the JSON form `dataclasses.asdict` or `model_dump_json(by_alias=True)`
  writes. Elixir names exported fields by key, which is the same unless a
  field is renamed.
- **PP6. `Tagged` is internally tagged only**, with each variant named by
  a `Literal` field of its class, as pydantic's discriminated unions are,
  and pydantic's `Field(discriminator=...)` is read as `Tagged`. A class
  with only the tag is a unit variant. Only this style agrees with the
  classes' own digests, which include the tag as a field; external and
  adjacent styles can be written in an identity's JSON form.
- **PP7. Map keys that are the same data-model value** (`"a"` and `b"a"`,
  an enum member and its value) are an error, as Elixir's atom and string
  keys are, rather than a map with a repeated key.
- **PP8. `parse_json(text)` is public**: JSON text with numbers as spec 2.7
  reads them, and repeated member names an error. `json.loads` differs for
  integers beyond 64 bits, which it keeps exact, so digests of its output
  can differ from other languages'. `encoding(value)` is public too (PY4).
- **PP9. `@digestible` keeps the names where its class was defined**, as
  pydantic keeps a model's parent namespace, so that a class defined in a
  function resolves its type hints under `from __future__ import
  annotations`. An undecorated class whose hints cannot be resolved is an
  error that says so, never a silent `any`.
- **PP10. pydantic details (PY8):** a `field_serializer` is applied through
  pydantic's own serializer for that one field (`to_python(include=...)`),
  in JSON mode when the serializer is for JSON only. A model with a
  `model_serializer`, and a `RootModel`, is digested as it serializes
  itself, in Python mode and by alias, as Rust bridges a custom `Serialize`
  through serde; such a model cannot be `@digestible`. Extra fields
  (`extra="allow"`) do not count, as unlisted members do not.
- **PP11. Computed fields do not count.** PY8 said "only when included",
  but nothing includes one yet; they are derived from fields that do
  count. To revisit with a marker or a decorator option.
- **PP12. Text and bytes are different source types.** In the data model
  both are strings (D17), but under an identity a `bytes` value is the
  `bytes` source type, which `"string"` does not accept, as spec 5.2's
  source types say.
- **PP13. `Identity` is immutable, hashable and picklable**, compared by
  its canonical JSON form. Besides `quiddity.digest(value, identity)`, it
  has `identity.digest(value)` and `identity.read(value)` (the data-model
  value), as Rust's `Identity::digest`.
- **PP14. Error paths in identities name each kind**:
  `$.definitions.Task.object.fields.title`, where Elixir writes
  `$.definitions.Task.fields.title`. To align when the spec says how a
  path is written.
- **PP15. A `TypedDict` is declarable**: `@digestible` gives it a domain and
  `rename_all` as an identity, and `NotRequired` members are optional
  (`omit: ["none"]`), read from the resolved hints so that they work under
  `from __future__ import annotations`.
- **PP16. A union's alternatives are ordered by their canonical JSON
  text** in `to_json()`, which may differ from Rust's order; the order is
  not part of any digest (Appendix C reads a union as a set).

### Not done yet

- The tests run on 3.11, 3.12 and 3.14 here; 3.13 is not installed.
- `identity_of` for a class with `__quiddity__` exports `any`, since its
  form is not declared.
- msgspec `Struct`s are not declared classes.
- Out of scope, as `ndarray` and `polars` are for Rust (DESIGN.md, D10):
  NumPy arrays and pandas and Polars data frames. Their mapping onto the
  data model is ambiguous, and a per-value encoding is the wrong tool for
  millions of elements; their raw buffers can be hashed instead. NumPy
  scalars (`np.int64`) are single values and could be supported.
- Speed: about 6 µs per row of five fields, 13 under an identity, in pure
  Python. A compiled encoder could follow if needed.
