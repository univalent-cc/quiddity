"""Digests of declared classes: dataclasses with Annotated markers.

The same book as the Elixir package's examples/structs.exs, which prints the
same digests.

    python examples/declared.py
"""

from __future__ import annotations

import dataclasses
import json
from dataclasses import dataclass
from datetime import UTC, datetime
from typing import Annotated

import quiddity as q


@dataclass
class Author:
    name: str
    born: int


@q.digestible(domain="example.book", identities=["meaning"])
@dataclass
class Book:
    title: str
    authors: list[Author]
    # Counts for the primary identity, not for "meaning": the work, whatever
    # the edition.
    pages: Annotated[int, q.Skip("meaning")]
    # The order of the genres does not count.
    genres: Annotated[list[str], q.AsSet()]
    # When the library record was made, rounded down to the second.
    catalogued_at: Annotated[datetime, q.Quantize(1_000_000_000)]
    # Rounded down to the cent.
    price: Annotated[float, q.Decimals(2)]
    # Left out when None, so that adding it did not change existing digests.
    subtitle: Annotated[str | None, q.OmitNone()] = None
    # When you last read it: not part of the book, so it never counts.
    last_opened: Annotated[datetime | None, q.Skip()] = None


book = Book(
    title="Moby Dick",
    authors=[Author("Herman Melville", 1819)],
    pages=635,
    genres=["novel", "adventure"],
    catalogued_at=datetime(2026, 10, 15, 9, 30, 0, 250_000, tzinfo=UTC),
    price=12.5,
    last_opened=datetime.now(UTC),
)

digest = q.digest(book)
print(f"primary:   {digest}")
print(f"meaning:   {q.digest(book, 'meaning')}")

# Fields that do not count, or that differ by less than the grid, or in
# order only, leave the digest unchanged.
same = dataclasses.replace(book, last_opened=None, genres=["adventure", "novel"], price=12.501)
print(f"unchanged: {q.digest(same) == digest}")

# A change in pages changes the primary digest, not meaning's.
abridged = dataclasses.replace(book, pages=320)
print(f"primary changed: {q.digest(abridged) != digest}")
print(f"meaning same:    {q.digest(abridged, 'meaning') == q.digest(book, 'meaning')}")

# The class's identity as data, for another language to apply to its JSON
# form. Kinds come from the type hints.
identity = q.identity_of(Book)
print(f"\nidentity as JSON:\n{json.dumps(identity.to_json())}\n")

# The JSON form, as a web framework would write it.
text = json.dumps(
    dataclasses.asdict(book),
    default=datetime.isoformat,
)
print(f"JSON form digests alike: {identity.digest(json.loads(text)) == digest}")
