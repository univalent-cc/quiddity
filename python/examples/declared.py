"""Digests of declared classes: dataclasses with Annotated markers.

The same task as the Elixir package's examples/structs.exs, which prints the
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
class DocumentRef:
    label: str
    content: q.Digest


@q.digestible(domain="example.task", identities=["meaning"])
@dataclass
class Task:
    prompt: str
    documents: list[DocumentRef]
    # Counts for the primary identity, not for "meaning".
    priority: Annotated[int, q.Skip("meaning")]
    # The order of the tags does not count.
    tags: Annotated[list[str], q.AsSet()]
    # Rounded down to the second.
    asked_at: Annotated[datetime, q.Quantize(1_000_000_000)]
    # Rounded down to the cent.
    budget: Annotated[float, q.Decimals(2)]
    # Left out when None, so that adding it did not change existing digests.
    note: Annotated[str | None, q.OmitNone()] = None
    # Never counts.
    submitted_at: Annotated[datetime | None, q.Skip()] = None


task = Task(
    prompt="Summarize the report",
    documents=[DocumentRef("report", q.digest("The report's text"))],
    priority=1,
    tags=["finance", "q3"],
    asked_at=datetime(2026, 10, 15, 9, 30, 0, 250_000, tzinfo=UTC),
    budget=12.5,
    submitted_at=datetime.now(UTC),
)

digest = q.digest(task)
print(f"primary:   {digest}")
print(f"meaning:   {q.digest(task, 'meaning')}")

# Fields that do not count, or that differ by less than the grid, or in
# order only, leave the digest unchanged.
same = dataclasses.replace(task, submitted_at=None, tags=["q3", "finance"], budget=12.501)
print(f"unchanged: {q.digest(same) == digest}")

# A change in priority changes the primary digest, not meaning's.
urgent = dataclasses.replace(task, priority=9)
print(f"primary changed: {q.digest(urgent) != digest}")
print(f"meaning same:    {q.digest(urgent, 'meaning') == q.digest(task, 'meaning')}")

# The class's identity as data, for another language to apply to its JSON
# form. Kinds come from the type hints.
identity = q.identity_of(Task)
print(f"\nidentity as JSON:\n{json.dumps(identity.to_json())}\n")

# The JSON form, as a web framework would write it.
text = json.dumps(
    dataclasses.asdict(task),
    default=lambda o: o.isoformat() if isinstance(o, datetime) else str(o),
)
print(f"JSON form digests alike: {identity.digest(json.loads(text)) == digest}")
