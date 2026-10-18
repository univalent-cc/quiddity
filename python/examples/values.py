"""Digests of Python values.

python examples/values.py
"""

from datetime import UTC, datetime, timedelta, timezone

import quiddity as q

# Two values that mean the same thing have the same digest: dict keys in any
# order, 1 and 1.0, a datetime in any time zone.
a = q.digest(
    {
        "title": "Moby Dick",
        "pages": 635,
        "published": datetime(1851, 10, 18, tzinfo=UTC),
    }
)
b = q.digest(
    {
        "published": datetime(1851, 10, 18, 2, tzinfo=timezone(timedelta(hours=2))),
        "pages": 635.0,
        "title": "Moby Dick",
    }
)
print(f"digest:         {a}")
print(f"same value:     {a == b}")

# A set's order does not count; a list's does.
print(f"sets:           {q.digest({1, 2}) == q.digest({2, 1})}")
print(f"lists:          {q.digest([1, 2]) == q.digest([2, 1])}")

# What has no data-model value is an error, which says where. A naive
# datetime is one: it means a different instant on each machine.
try:
    q.digest({"steps": [1, datetime(2026, 10, 15, 9, 30)]})
except q.QuiddityError as e:
    print(f"error:          {e.path}: {e.message[:42]}…")

# Shorter forms, for an ETag for example, and the text form read back.
print(f"micro:          {a.micro()}")
print(f"parsed back:    {q.Digest.parse(str(a)) == a}")

# A domain keeps digests of different kinds of values apart.
payment = q.digest({"to": "bob", "amount": 100}).in_domain("example.payment")
refund = q.digest({"to": "bob", "amount": 100}).in_domain("example.refund")
print(f"domains differ: {payment != refund}")
