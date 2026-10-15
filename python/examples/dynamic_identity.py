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


class DocumentRef(TypedDict):
    label: str
    content: str


# A union of classes told apart by their "type" member: a class with only the
# tag is a unit variant.
class Free(TypedDict):
    type: Literal["Free"]


class Paragraphs(TypedDict):
    type: Literal["Paragraphs"]
    count: int


@q.digestible(domain="example.task")
class Task(TypedDict):
    prompt: str
    documents: list[DocumentRef]
    priority: int
    format: Annotated[Free | Paragraphs, q.Tagged("type")]
    # Left out when null or missing.
    note: NotRequired[str | None]
    # Left out when empty; the order of its members does not count.
    tags: Annotated[set[str], q.OmitEmpty()]
    # Read from the member askedAt, digested under the key asked_at, and
    # rounded down to the second.
    askedAt: Annotated[datetime, q.Rename("asked_at"), q.Quantize(1_000_000_000)]
    # Another task, or null.
    follows: NotRequired[Task | None]


task = q.Identity(Task)

# --- Digesting ------------------------------------------------------------------

value = {
    "prompt": "Summarize the report",
    "documents": [{"label": "report", "content": "sha256:9f2c…"}],
    "priority": 1,
    "format": {"type": "Paragraphs", "count": 2},
    "tags": ["finance", "q3"],
    "askedAt": "2026-10-14T09:30:00.250Z",
    # Not a field: skipped.
    "submitted_by": "alice",
}

digest = task.digest(value)
print(f"digest:            {digest}")

# The same task, written differently: members in another order, the integer
# as text, the instant in another time zone, the set reordered.
same = {
    "askedAt": "2026-10-14T11:30:00+02:00",
    "tags": ["q3", "finance"],
    "note": None,
    "format": {"count": 2, "type": "Paragraphs"},
    "priority": "1",
    "documents": [{"content": "sha256:9f2c…", "label": "report"}],
    "prompt": "Summarize the report",
}
print(f"written differently: same digest: {task.digest(same) == digest}")

# A value of the wrong shape is an error that says where.
wrong = {**value, "documents": [{"label": 7, "content": "x"}]}
try:
    task.digest(wrong)
except q.QuiddityError as e:
    print(f"wrong shape:       at {e.path}: {e.message}")

# --- Changing -------------------------------------------------------------------

# A change that would make the identity invalid raises, and changes nothing.
try:
    task.with_field_in("Task", "title", Annotated[str, q.Rename("prompt")])
except q.InvalidIdentity as e:
    print(f"invalid change:    {e}")

task = task.without_field_in("Task", "priority")
print(f"without priority:  {task.digest(value)}")

# --- As JSON --------------------------------------------------------------------

# An identity is data: written out as JSON, any implementation reads it.
loaded = q.Identity.from_json(json.dumps(task.to_json()))
print(f"loaded back:       {loaded == task}")

# An identity has a digest of its own, the same in every implementation.
print(f"identity's digest: {q.digest(task)}")
