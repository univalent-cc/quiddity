"""Identities as data (history/PYTHON.md, PY6 and PY7)."""

import enum
import json
import pickle
import uuid
from collections.abc import Mapping, Sequence
from datetime import UTC, date, datetime, timedelta
from decimal import Decimal
from pathlib import Path
from typing import Annotated, Any, Literal, NamedTuple, NotRequired, Optional, TypedDict

import pytest

import quiddity as q


def kind(tp: Any) -> Any:
    return q.Identity(tp).to_json()["root"]


class Color(enum.Enum):
    RED = "red"
    BLUE = "blue"


class Pair(NamedTuple):
    a: int
    b: str


def test_types_as_kinds() -> None:
    cases: list[tuple[Any, Any]] = [
        (int, "integer"),
        (float, "number"),
        (str, "string"),
        (bool, "bool"),
        (None, "null"),
        (Any, "any"),
        (bytes, {"bytes": {}}),
        (Annotated[bytes, q.Hex()], {"bytes": {"text": "hex"}}),
        (datetime, {"instant": {}}),
        (Annotated[datetime, q.Unit("ms")], {"instant": {"unit": "ms"}}),
        (timedelta, {"duration": {}}),
        (uuid.UUID, "uuid"),
        (Path, "path"),
        (date, "string"),
        (Decimal, "string"),
        (q.Digest, "string"),
        (list[str], {"list": "string"}),
        (Sequence[int], {"list": "integer"}),
        (tuple[int, ...], {"list": "integer"}),
        (tuple[int, str], {"tuple": ["integer", "string"]}),
        (set[str], {"set": "string"}),
        (frozenset[int], {"set": "integer"}),
        (dict[str, int], {"map": {"key": "string", "value": "integer"}}),
        (Mapping[int, Any], {"map": {"key": "integer", "value": "any"}}),
        (Optional[str], {"union": ["null", "string"]}),  # noqa: UP045
        (str | int, {"union": ["integer", "string"]}),
        # Not a valid union (two numbers): read as it is.
        (int | float, "any"),
        (Literal["a", "b"], "string"),
        (Literal[1, None], {"union": ["integer", "null"]}),
        (Color, "string"),
        (Pair, {"tuple": ["integer", "string"]}),
        (object, "any"),
    ]
    for tp, expected in cases:
        assert kind(tp) == expected, tp


def test_conversions() -> None:
    at = q.Identity(Annotated[datetime, q.Unit("s")])
    assert at.digest(1_700_000_000) == at.digest("2023-11-14T22:13:20Z")
    assert at.digest(datetime(2023, 11, 14, 22, 13, 20, tzinfo=UTC)) == at.digest("1700000000")
    hexes = q.Identity(Annotated[bytes, q.Hex()])
    assert hexes.digest("0102") == hexes.digest(b"\x01\x02") == hexes.digest([1, 2])
    sets = q.Identity(set[int])
    assert sets.digest([2, 1, "3"]) == sets.digest({1, 2, 3})
    with pytest.raises(q.QuiddityError, match="does not accept a null") as e:
        q.Identity(list[int]).digest([1, "2", None])
    assert e.value.path == "$[2]"
    with pytest.raises(q.QuiddityError, match="not an integer") as e:
        q.Identity(list[int]).digest([1, "x"])
    assert e.value.path == "$[1]"


class Row(TypedDict):
    label: str
    value: float


class Doc(TypedDict):
    rows: list[Row]
    note: NotRequired[str | None]
    at: Annotated[datetime, q.Rename("written_at")]


def test_typed_dicts_nest_by_name() -> None:
    doc = q.Identity(Doc)
    j = doc.to_json()
    assert j["root"] == {"ref": "Doc"}
    assert set(j["definitions"]) == {"Doc", "Row"}
    assert j["definitions"]["Doc"]["object"]["fields"]["at"] == {
        "kind": {"instant": {}},
        "rename": "written_at",
    }
    value = {"rows": [{"label": "x", "value": 1, "extra": True}], "at": "2024-01-01T00:00:00Z"}
    assert doc.digest(value) == q.digest(
        {
            "rows": [{"label": "x", "value": 1}],
            "written_at": datetime(2024, 1, 1, tzinfo=UTC),
        }
    )


def test_identities_nest() -> None:
    row = q.Identity(dict[str, int]).named("Cells")
    table = q.Identity(list[row])  # type: ignore[valid-type]
    assert table.to_json() == {
        "root": {"list": {"ref": "Cells"}},
        "definitions": {"Cells": {"map": {"key": "string", "value": "integer"}}},
    }
    assert row.name == "Cells" and table.name is None
    renamed = row.named("Row")
    assert renamed.to_json()["definitions"] == {
        "Row": {"map": {"key": "string", "value": "integer"}}
    }


def test_changes() -> None:
    base = q.Identity.from_json({"root": {"object": {"fields": {"a": {"kind": "string"}}}}})
    changed = base.with_field("n", Annotated[int, q.OmitNone(), q.Quantize(10)])
    assert changed.to_json()["root"]["object"]["fields"]["n"] == {
        "kind": {"union": ["integer", "null"]},
        "omit": ["none"],
        "quantize": 10,
    }
    assert changed.digest({"a": "x", "n": 17}) == changed.digest({"a": "x", "n": "10"})
    assert changed.without_field("n") == base
    with pytest.raises(q.InvalidIdentity, match="two fields have the key"):
        base.with_field("b", Annotated[str, q.Rename("a")])
    with pytest.raises(q.InvalidIdentity, match="no field"):
        base.without_field("z")
    with pytest.raises(q.InvalidIdentity, match="not an object"):
        q.Identity(int).with_field("x", int)
    doc = q.Identity(Doc)
    assert (
        "note"
        not in doc.without_field_in("Doc", "note").to_json()["definitions"]["Doc"]["object"][
            "fields"
        ]
    )
    assert doc.with_domain("example.doc").domain == "example.doc"
    value = {"rows": [], "at": "1970-01-01T00:00:00Z"}
    assert doc.with_domain("example.doc").digest(value) == doc.digest(value).in_domain(
        "example.doc"
    )


def test_identities_are_values() -> None:
    a = q.Identity(list[str])
    b = q.Identity.from_json(json.dumps({"root": {"list": "string"}}))
    assert a == b and hash(a) == hash(b)
    assert pickle.loads(pickle.dumps(a)) == a
    with pytest.raises(AttributeError):
        a._root = 1  # type: ignore[misc]
    assert q.digest(a) == q.digest(b)
    assert q.digest(a) != q.digest(q.Identity(list[int]))
    assert "Identity.from_json" in repr(a)
    with pytest.raises(q.QuiddityError):
        q.digest(a, "meaning")


def test_json_errors() -> None:
    with pytest.raises(q.InvalidIdentity, match="repeated"):
        q.Identity.from_json('{"root": "string", "root": "integer"}')
    with pytest.raises(q.InvalidIdentity, match="not JSON"):
        q.Identity.from_json("{")
    with pytest.raises(q.InvalidIdentity, match="unknown member 'quiddity'"):
        q.Identity.from_json('{"quiddity": 1, "root": "string"}')
    with pytest.raises(q.InvalidIdentity, match="quantize") as e:
        q.Identity.from_json(
            '{"root": {"object": {"fields": {"n": {"kind": "integer", "quantize": 5.0}}}}}'
        )
    assert e.value.path == "$.root.object.fields.n.quantize"


class Circle(TypedDict):
    type: Literal["circle"]
    radius: float


class Dot(TypedDict):
    type: Literal["dot"]


def test_tagged_unions() -> None:
    shape = q.Identity(Annotated[Circle | Dot | None, q.Tagged("type")])
    assert shape.to_json()["root"] == {
        "union": [
            "null",
            {
                "tagged": {
                    "style": "internal",
                    "tag": "type",
                    "variants": {
                        "circle": {"kind": {"object": {"fields": {"radius": {"kind": "number"}}}}},
                        "dot": {},
                    },
                }
            },
        ]
    }
    assert shape.digest({"type": "circle", "radius": 1}) == q.digest(
        {"type": "circle", "radius": 1}
    )
    assert shape.digest({"type": "dot"}) == q.digest({"type": "dot"})
    with pytest.raises(q.InvalidIdentity, match="Literal"):
        q.Identity(Annotated[Circle | Row, q.Tagged("type")])
