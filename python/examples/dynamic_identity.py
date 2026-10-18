"""Identities built at run time, applied to values decoded from JSON.

The same identity as the Rust crate's examples/dynamic_identity.rs and the
Elixir package's examples/dynamic_identity.exs, which print the same
digests.

    python examples/dynamic_identity.py
"""

from __future__ import annotations

import json
from datetime import datetime
from typing import Annotated, Literal, NotRequired, TypedDict

import quiddity as q

# --- Building -------------------------------------------------------------------

# An identity is written as types. A class becomes a definition of the
# identities it is nested in, under its name.


class Author(TypedDict):
    name: str
    born: int


# A union of classes told apart by their "type" member: a class with only the
# tag is a unit variant.
class Ebook(TypedDict):
    type: Literal["Ebook"]


class Print(TypedDict):
    type: Literal["Print"]
    volumes: int


@q.digestible(domain="example.book")
class Book(TypedDict):
    title: str
    authors: list[Author]
    pages: int
    format: Annotated[Ebook | Print, q.Tagged("type")]
    # Left out when null or missing.
    subtitle: NotRequired[str | None]
    # Left out when empty; the order of its members does not count.
    genres: Annotated[set[str], q.OmitEmpty()]
    # When the library record was made: read from the member cataloguedAt,
    # digested under the key catalogued_at, and rounded down to the second.
    cataloguedAt: Annotated[datetime, q.Rename("catalogued_at"), q.Quantize(1_000_000_000)]
    # The book this one follows, or null.
    follows: NotRequired[Book | None]


book = q.Identity(Book)

# --- Digesting ------------------------------------------------------------------

value = {
    "title": "Moby Dick",
    "authors": [{"name": "Herman Melville", "born": 1819}],
    "pages": 635,
    "format": {"type": "Print", "volumes": 3},
    "genres": ["novel", "adventure"],
    "cataloguedAt": "2026-10-14T09:30:00.250Z",
    # Not a field: skipped.
    "last_opened": "2026-10-16T21:05:00Z",
}

digest = book.digest(value)
print(f"digest:            {digest}")

# The same book, written differently: members in another order, the integer
# as text, the instant in another time zone, the set reordered.
same = {
    "cataloguedAt": "2026-10-14T11:30:00+02:00",
    "genres": ["adventure", "novel"],
    "subtitle": None,
    "format": {"volumes": 3, "type": "Print"},
    "pages": "635",
    "authors": [{"born": 1819, "name": "Herman Melville"}],
    "title": "Moby Dick",
}
print(f"written differently: same digest: {book.digest(same) == digest}")

# A value of the wrong shape is an error that says where.
wrong = {**value, "authors": [{"name": 7, "born": 1819}]}
try:
    book.digest(wrong)
except q.QuiddityError as e:
    print(f"wrong shape:       at {e.path}: {e.message}")

# --- Changing -------------------------------------------------------------------

# A change that would make the identity invalid raises, and changes nothing.
try:
    book.with_field_in("Book", "name", Annotated[str, q.Rename("title")])
except q.InvalidIdentity as e:
    print(f"invalid change:    {e}")

book = book.without_field_in("Book", "pages")
print(f"without pages:     {book.digest(value)}")

# --- As JSON --------------------------------------------------------------------

# An identity is data: written out as JSON, any implementation reads it.
loaded = q.Identity.from_json(json.dumps(book.to_json()))
print(f"loaded back:       {loaded == book}")

# An identity has a digest of its own, the same in every implementation.
print(f"identity's digest: {q.digest(book)}")
