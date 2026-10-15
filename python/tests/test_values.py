"""Python values in the data model (history/PYTHON.md, PY3)."""

from __future__ import annotations

import calendar
import enum
import math
import uuid
from collections import OrderedDict
from datetime import UTC, date, datetime, time, timedelta, timezone
from decimal import Decimal
from pathlib import PurePosixPath
from typing import NamedTuple

import pytest

import quiddity as q
from quiddity._model import Duration, Instant, Map, Set


def same(a: object, b: object) -> bool:
    return q.digest(a) == q.digest(b)


def test_scalars() -> None:
    assert q.encoding(None).hex() == "01"
    assert q.encoding(False).hex() == "02"
    assert q.encoding(True).hex() == "03"
    assert same(1, 1.0)
    assert same(-0.0, 0)
    assert not same(True, 1)
    assert not same(1, "1")
    assert same(2**70, 2**70)
    assert q.encoding(math.nan) == q.encoding(-math.nan)
    assert not same(math.inf, -math.inf)


def test_text_and_bytes() -> None:
    # Text and bytes are one kind (D17): text is its UTF-8 bytes.
    assert same("é", "é".encode())
    assert same(bytearray(b"ab"), b"ab")
    assert same(memoryview(b"ab"), b"ab")
    # Lone surrogates are kept (spec 2.3); a pair is one code point.
    assert not same("\ud800", "")
    assert same("😀", "😀")


def test_collections() -> None:
    assert same((1, 2), [1, 2])
    assert not same([1, 2], [2, 1])
    assert same({1, 2}, frozenset({2, 1}))
    assert not same({1}, [1])
    assert same({"a": 1, "b": 2}, OrderedDict([("b", 2), ("a", 1)]))
    # Keys keep their kinds.
    assert not same({1: "a"}, {"1": "a"})
    with pytest.raises(q.QuiddityError, match="given twice"):
        q.digest({"a": 1, b"a": 2})


def test_times() -> None:
    utc = datetime(2026, 10, 15, 9, 30, tzinfo=UTC)
    plus2 = datetime(2026, 10, 15, 11, 30, tzinfo=timezone(timedelta(hours=2)))
    assert same(utc, plus2)
    assert same(utc, Instant(calendar.timegm((2026, 10, 15, 9, 30, 0)) * 10**9))
    assert same(timedelta(days=1, microseconds=1), Duration(86_400 * 10**9 + 1000))
    assert same(timedelta(seconds=-1), -(10**9))
    with pytest.raises(q.QuiddityError, match="naive datetime") as e:
        q.digest({"at": [datetime(2026, 10, 15)]})
    assert e.value.path == "$.at[0]"
    assert same(date(2026, 10, 15), "2026-10-15")
    assert same(time(9, 30), "09:30:00")


def test_strings_of_other_types() -> None:
    assert same(Decimal("12.50"), "12.50")
    assert same(Decimal("1E+10"), "1E+10")
    u = uuid.UUID("12345678-1234-5678-1234-567812345678")
    assert same(u, "12345678-1234-5678-1234-567812345678")
    assert same(PurePosixPath("/tmp/a"), "/tmp/a")
    d = q.digest("x")
    assert same(d, str(d))


class Color(enum.Enum):
    RED = "red"
    GREEN = 2


class Pair(NamedTuple):
    a: int
    b: str


def test_enums_and_named_tuples() -> None:
    assert same(Color.RED, "red")
    assert same(Color.GREEN, 2)
    assert same(Pair(1, "x"), [1, "x"])


def test_errors_say_where() -> None:
    with pytest.raises(q.QuiddityError) as e:
        q.digest({"steps": [1, object()]})
    assert e.value.path == "$.steps[1]"
    assert "object has no data-model value" in e.value.message
    assert isinstance(e.value, ValueError)


def test_model_values_pass_through() -> None:
    assert same(Set([1, 1]), Set([1, 1]))
    assert not same(Set([1, 1]), {1})
    assert same(Map([("a", 1)]), {"a": 1})


def test_digest() -> None:
    d = q.digest([1])
    assert d.bits == 256 and len(d.bytes) == 32
    assert d.mini().bits == 128 and d.micro().bits == 64
    assert q.Digest.parse(str(d.mini())) == d.mini()
    assert q.Digest.parse(str(d).upper().replace("SHA256", "sha256")) == d
    with pytest.raises(ValueError):
        q.Digest.parse("sha256:00")
    with pytest.raises(ValueError):
        d.micro().in_domain("x")
    with pytest.raises(ValueError):
        d.in_domain("")
    assert d.in_domain("a") != d.in_domain("b")
    assert len({d, q.Digest(d.bytes)}) == 1
    assert sorted([d.micro(), d]) == [d.micro(), d]
    with pytest.raises(AttributeError):
        d.x = 1  # type: ignore[attr-defined]
    import pickle

    assert pickle.loads(pickle.dumps(d)) == d


def test_parse_json() -> None:
    assert q.parse_json("[1.0, 1e0, -0]") == [1, 1, 0]
    assert q.parse_json("18446744073709551616") == 2**64
    assert type(q.parse_json("18446744073709551617")) is int  # 2^64, after rounding
    with pytest.raises(q.QuiddityError, match="repeated"):
        q.parse_json('{"a": 1, "a": 2}')
    with pytest.raises(q.QuiddityError):
        q.parse_json("NaN")
