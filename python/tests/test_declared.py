"""Declared classes: dataclasses, attrs classes and TypedDicts, with markers
(history/PYTHON.md, PY5), and the identities they export."""

import dataclasses
import json
from dataclasses import dataclass, field
from datetime import UTC, datetime, timedelta
from typing import Annotated, Any

import attrs
import pytest

import quiddity as q


@dataclass
class Plain:
    a: int
    b: str


def test_a_plain_dataclass_is_its_fields() -> None:
    assert q.digest(Plain(1, "x")) == q.digest({"a": 1, "b": "x"})
    with pytest.raises(q.QuiddityError, match="no identity 'meaning'"):
        q.digest(Plain(1, "x"), "meaning")


@q.digestible(domain="example.book", identities=["meaning"])
@dataclass
class Book:
    title: str
    pages: Annotated[int, q.Skip("meaning")]
    genres: Annotated[list[str], q.AsSet()]
    catalogued_at: Annotated[datetime, q.Quantize(1_000_000_000)]
    price: Annotated[float, q.Decimals(2)] = 0.0
    subtitle: Annotated[str | None, q.OmitNone()] = None
    last_opened: Annotated[datetime | None, q.Skip()] = None
    kind: Annotated[str, q.Rename("type"), q.Rename("sort", "meaning")] = "q"


def book(**changes: Any) -> Book:
    b = Book(
        "Moby Dick",
        635,
        ["novel", "adventure"],
        datetime(2026, 1, 1, 0, 0, 0, 250_000, tzinfo=UTC),
        12.5,
    )
    return dataclasses.replace(b, **changes)


def test_markers() -> None:
    expected = {
        "title": "Moby Dick",
        "pages": 635,
        "genres": q._model.Set(["novel", "adventure"]),
        "catalogued_at": datetime(2026, 1, 1, tzinfo=UTC),
        "price": 12.5,
        "type": "q",
    }
    assert q.digest(book()) == q.digest(expected).in_domain("example.book")
    assert q.digest(
        book(genres=["adventure", "novel"], price=12.509, last_opened=datetime.now(UTC))
    ) == q.digest(book())
    assert q.digest(book(subtitle="or, The Whale")) != q.digest(book())


def test_scoped_markers() -> None:
    meaning = q.digest(book(), "meaning")
    assert q.digest(book(pages=720), "meaning") == meaning
    assert q.digest(book(pages=720)) != q.digest(book())
    expected = q.digest(
        {
            "title": "Moby Dick",
            "genres": q._model.Set(["novel", "adventure"]),
            "catalogued_at": datetime(2026, 1, 1, tzinfo=UTC),
            "price": 12.5,
            "sort": "q",
        }
    ).in_domain("example.book")
    assert meaning == expected


def test_nested_and_flatten() -> None:
    @q.digestible(identities=["short"])
    @dataclass
    class Inner:
        x: int
        y: Annotated[int, q.Skip("short")] = 0

    @dataclass
    class Outer:
        inner: Inner
        short: Annotated[Inner | None, q.Nested("short")]
        flat: Annotated[Inner, q.Flatten()]
        extra: Annotated[dict[str, int] | None, q.Flatten()] = None

    o = Outer(Inner(1, 2), Inner(3, 4), Inner(5, 6), {"z": 7})
    assert q.digest(o) == q.digest(
        {"inner": {"x": 1, "y": 2}, "short": {"x": 3}, "x": 5, "y": 6, "z": 7}
    )
    assert q.digest(dataclasses.replace(o, short=None, extra=None)) == q.digest(
        {"inner": {"x": 1, "y": 2}, "short": None, "x": 5, "y": 6}
    )
    with pytest.raises(q.QuiddityError, match="same name") as e:
        q.digest(dataclasses.replace(o, extra={"x": 0}))
    assert e.value.path == "$"


def test_rename_all() -> None:
    @q.digestible(rename_all="camelCase")
    @dataclass
    class C:
        first_name: str
        last_name: Annotated[str, q.Rename("surname")]

    assert q.digest(C("a", "b")) == q.digest({"firstName": "a", "surname": "b"})
    from quiddity._declared import apply_rule

    rules = {
        "PascalCase": "FirstName",
        "kebab-case": "first-name",
        "SCREAMING_SNAKE_CASE": "FIRST_NAME",
        "SCREAMING-KEBAB-CASE": "FIRST-NAME",
        "UPPERCASE": "FIRST_NAME",
        "lowercase": "first_name",
    }
    for rule, name in rules.items():
        assert apply_rule("first_name", rule) == name


def test_errors_in_declarations() -> None:
    with pytest.raises(TypeError, match="does not declare"):

        @dataclass
        class Bad:
            a: Annotated[int, q.Skip("meaning")]

        q.digest(Bad(1))
    with pytest.raises(TypeError, match="both apply"):

        @dataclass
        class Twice:
            a: Annotated[int, q.Rename("x"), q.Rename("y")]

        q.digest(Twice(1))
    with pytest.raises(TypeError, match="not both"):

        @dataclass
        class Both:
            a: Annotated[int, q.Quantize(2), q.Decimals(1)]

        q.digest(Both(1))
    with pytest.raises(TypeError, match="two fields have the key"):

        @dataclass
        class Clash:
            a: int
            b: Annotated[int, q.Rename("a")]

        q.digest(Clash(1, 2))
    with pytest.raises(TypeError, match="above @dataclass"):
        q.digestible(Plain.__init__)  # type: ignore[arg-type]
    with pytest.raises(TypeError):
        q.Quantize(0)
    with pytest.raises(TypeError):
        q.digestible(identities=["primary"])


def test_wrong_values_for_markers() -> None:
    @dataclass
    class C:
        n: Annotated[Any, q.Quantize(10)]
        s: Annotated[Any, q.AsSet()] = None

    with pytest.raises(q.QuiddityError, match="quantize applies") as e:
        q.digest(C("x"))
    assert e.value.path == "$.n"
    with pytest.raises(q.QuiddityError, match="AsSet"):
        q.digest(C(1, "abc"))
    assert q.digest(C(15, None)) == q.digest({"n": 10, "s": None})
    assert q.digest(C(timedelta(seconds=1, microseconds=5))) == q.digest(
        {
            "n": timedelta(seconds=1, microseconds=5)
            // timedelta(microseconds=1)
            * 1000
            // 10
            * 10,
            "s": None,
        }
    )


def test_attrs_classes() -> None:
    @q.digestible(identities=["meaning"])
    @attrs.define
    class A:
        x: int
        y: Annotated[int, q.Skip("meaning")] = 0

    assert q.digest(A(1, 2)) == q.digest({"x": 1, "y": 2})
    assert q.digest(A(1, 2), "meaning") == q.digest({"x": 1})


def test_hand_written() -> None:
    class Interval:
        __quiddity_domain__ = "example.interval"

        def __init__(self, start: int, length: int) -> None:
            self.start, self.length = start, length

        def __quiddity__(self, identity: str) -> object:
            return {"start": self.start, "end": self.start + self.length}

    assert q.digest(Interval(1, 2)) == q.digest({"start": 1, "end": 3}).in_domain(
        "example.interval"
    )
    # Nested, it has no domain.
    assert q.digest([Interval(1, 2)]) == q.digest([{"start": 1, "end": 3}])


def test_local_forward_references() -> None:
    @dataclass
    class Node:
        children: "list[Node]" = field(default_factory=list)

    with pytest.raises(TypeError, match="cannot be resolved"):
        q.digest(Node())

    # A decorated class sees the names where it was defined, itself among them.
    @q.digestible
    @dataclass
    class Tree:
        children: "list[Tree]" = field(default_factory=list)

    assert q.digest(Tree([Tree()])) == q.digest({"children": [{"children": []}]})


# --- Export: a class's digest equals its JSON form's under its identity -------------


@dataclass
class Row:
    label: str
    value: float


@q.digestible(domain="example.review", rename_all="camelCase")
@dataclass
class Review:
    summary: str
    rows: list[Row]
    written_at: Annotated[datetime, q.Quantize(1_000)]
    tags: Annotated[list[str], q.AsSet(), q.OmitEmpty()] = field(default_factory=list)
    note: Annotated[str | None, q.OmitNone()] = None
    parent: "Review | None" = None


def json_form(x: Any) -> Any:
    def default(o: Any) -> Any:
        if isinstance(o, datetime):
            return o.isoformat()
        return str(o)

    return json.loads(json.dumps(dataclasses.asdict(x), default=default))


def test_exported_identity() -> None:
    identity = q.identity_of(Review)
    j = identity.to_json()
    assert j["domain"] == "example.review"
    assert j["root"] == {"ref": "Review"}
    fields = j["definitions"]["Review"]["object"]["fields"]
    assert fields["written_at"] == {
        "kind": {"instant": {}},
        "rename": "writtenAt",
        "quantize": 1000,
    }
    assert fields["tags"] == {
        "kind": {"union": ["null", {"set": "string"}]},
        "omit": ["empty", "none"],
    }
    assert fields["parent"] == {"kind": {"union": ["null", {"ref": "Review"}]}}
    assert j["definitions"]["Row"] == {
        "object": {"fields": {"label": {"kind": "string"}, "value": {"kind": "number"}}}
    }
    assert q.Identity(Review) == identity


def test_exported_identity_agrees_with_digests() -> None:
    identity = q.identity_of(Review)
    at = datetime(2026, 1, 1, 0, 0, 0, 123_456, tzinfo=UTC)
    for value in [
        Review("b", [Row("x", 1.5)], at),
        Review("b", [], at, ["t", "s"], "n", Review("p", [], at)),
    ]:
        assert identity.digest(json_form(value)) == q.digest(value)


def test_typed_dicts() -> None:
    from typing import NotRequired, TypedDict

    class Point(TypedDict):
        x: int
        y: NotRequired[int]

    assert q.Identity(Point).to_json()["definitions"]["Point"] == {
        "object": {
            "fields": {
                "x": {"kind": "integer"},
                "y": {"kind": {"union": ["integer", "null"]}, "omit": ["none"]},
            }
        }
    }
    # A TypedDict's values are dicts, digested as dicts.
    assert q.digest(Point(x=1)) == q.digest({"x": 1})
