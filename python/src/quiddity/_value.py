"""Python values as values of the data model (spec 2), as ``history/PYTHON.md``
PY3 maps them."""

from __future__ import annotations

import datetime as dt
import os
import uuid
from collections.abc import Mapping
from decimal import Decimal
from enum import Enum
from pathlib import PurePath
from typing import Any

from . import _declared
from ._digest import Digest
from ._errors import QuiddityError
from ._model import Duration, Instant, Map, Set, encode, normalize

_EPOCH = dt.datetime(1970, 1, 1, tzinfo=dt.UTC)


def instant_ns(d: dt.datetime, path: str = "$") -> int:
    """An aware datetime as integer nanoseconds since the Unix epoch."""
    if d.tzinfo is None or d.utcoffset() is None:
        raise QuiddityError(
            "a naive datetime has no instant: it means a different time on each machine; "
            "give it a time zone (tzinfo=datetime.timezone.utc, or zoneinfo.ZoneInfo(...))",
            path,
        )
    delta = d - _EPOCH
    return (delta.days * 86400 + delta.seconds) * 10**9 + delta.microseconds * 1000


def duration_ns(d: dt.timedelta) -> int:
    return (d.days * 86400 + d.seconds) * 10**9 + d.microseconds * 1000


def from_python(x: Any, path: str = "$") -> Any:
    """The data-model value of ``x``, raising ``QuiddityError`` if it has
    none. Declared classes are read under their primary identity."""
    t = type(x)
    if x is None or t is bool or t is int or t is str or t is bytes:
        return x
    if t is float:
        return normalize(x)
    if t is list or t is tuple:
        return [from_python(item, f"{path}[{i}]") for i, item in enumerate(x)]
    if t is dict:
        return _map(x, path)
    return _other(x, path)


def _other(x: Any, path: str) -> Any:
    t = type(x)
    if t is Map or t is Set or t is Instant or t is Duration:
        return x
    plan_value = _declared.read(x, "primary", path)
    if plan_value is not _declared.NOT_DECLARED:
        return plan_value
    if isinstance(x, Enum):
        return from_python(x.value, path)
    if isinstance(x, int):
        return int(x)
    if isinstance(x, float):
        return normalize(float(x))
    if isinstance(x, str):
        return str(x)
    if isinstance(x, (bytes, bytearray, memoryview)):
        return bytes(x)
    if isinstance(x, (list, tuple)):
        return [from_python(item, f"{path}[{i}]") for i, item in enumerate(x)]
    if isinstance(x, (set, frozenset)):
        return Set([from_python(item, f"{path}[]") for item in x])
    if isinstance(x, Mapping):
        return _map(x, path)
    if isinstance(x, dt.datetime):
        return Instant(instant_ns(x, path))
    if isinstance(x, (dt.date, dt.time)):
        return x.isoformat()
    if isinstance(x, dt.timedelta):
        return Duration(duration_ns(x))
    if isinstance(x, Decimal):
        # The General Decimal Arithmetic "to-scientific-string", as pydantic
        # writes it, and as Elixir's Decimal.to_string/1 does.
        return str(x)
    if isinstance(x, uuid.UUID):
        return str(x)
    if isinstance(x, PurePath):
        b = os.fsencode(x)
        try:
            return b.decode("utf-8")
        except UnicodeDecodeError:
            return b
    if isinstance(x, Digest):
        return str(x)
    raise QuiddityError(f"a {type(x).__qualname__} has no data-model value", path)


def _key_path(path: str, key: Any) -> str:
    return f"{path}.{key}" if type(key) is str else f"{path}[{key!r}]"


def _map(m: Mapping[Any, Any], path: str) -> Map:
    entries = []
    key_types = set()
    for k, v in m.items():
        key = from_python(k, path)
        key_types.add(type(key))
        entries.append((key, from_python(v, _key_path(path, key))))
    if len(key_types) > 1:
        check_keys(entries, path)
    return Map(entries)


def check_keys(entries: list[tuple[Any, Any]], path: str) -> None:
    """Raises if two keys are the same data-model value, as ``"a"`` and
    ``b"a"`` are, or an enum member and its value."""
    seen: dict[bytes, Any] = {}
    for k, _ in entries:
        e = encode(k)
        if e in seen:
            raise QuiddityError(f"the key {k!r} is given twice (also as {seen[e]!r})", path)
        seen[e] = k
