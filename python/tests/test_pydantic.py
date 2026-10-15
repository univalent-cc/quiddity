"""pydantic models (history/PYTHON.md, PY8): a model's digest follows its
configuration, and equals the digest of its JSON form under its identity."""

from datetime import UTC, datetime
from typing import Annotated, Any, Literal

import pytest

pydantic = pytest.importorskip("pydantic")

from pydantic import (  # noqa: E402
    BaseModel,
    ConfigDict,
    Field,
    RootModel,
    computed_field,
    field_serializer,
    model_serializer,
)
from pydantic.alias_generators import to_camel  # noqa: E402

import quiddity as q  # noqa: E402


class Circle(BaseModel):
    type: Literal["circle"] = "circle"
    radius: float


class Square(BaseModel):
    type: Literal["square"] = "square"
    side: float


@q.digestible(domain="example.drawing", identities=["meaning"])
class Drawing(BaseModel):
    model_config = ConfigDict(alias_generator=to_camel, populate_by_name=True)

    title: str
    created_at: Annotated[datetime, q.Quantize(10**9)]
    shapes: list[Annotated[Circle | Square, Field(discriminator="type")]]
    main_shape: Circle | Square = Field(discriminator="type")
    author: str = Field(serialization_alias="by")
    secret: str = Field(exclude=True)
    tags: Annotated[list[str], q.AsSet(), q.Skip("meaning")] = []
    note: Annotated[str | None, q.OmitNone()] = None

    @field_serializer("title")
    def upper(self, v: str) -> str:
        return v.upper()

    @computed_field  # type: ignore[prop-decorator]
    @property
    def area(self) -> float:
        return 1.0


def drawing(**changes: Any) -> Drawing:
    d = Drawing(
        title="t",
        created_at=datetime(2026, 1, 1, 0, 0, 0, 500_000, tzinfo=UTC),
        shapes=[Circle(radius=1), Square(side=2)],
        main_shape=Circle(radius=3),
        author="ada",
        secret="s",
        tags=["b", "a"],
    )
    return d.model_copy(update=changes)


def test_a_model_is_digested_as_configured() -> None:
    expected = {
        "title": "T",  # through its field_serializer
        "createdAt": datetime(2026, 1, 1, tzinfo=UTC),
        "shapes": [{"type": "circle", "radius": 1}, {"type": "square", "side": 2}],
        "mainShape": {"type": "circle", "radius": 3},
        "by": "ada",
        "tags": q._model.Set(["a", "b"]),
    }
    assert q.digest(drawing()) == q.digest(expected).in_domain("example.drawing")
    assert q.digest(drawing(secret="other", tags=["a", "b"])) == q.digest(drawing())
    assert q.digest(drawing(tags=[]), "meaning") == q.digest(drawing(), "meaning")


def test_the_json_form_digests_alike() -> None:
    identity = q.identity_of(Drawing)
    fields = identity.to_json()["definitions"]["Drawing"]["object"]["fields"]
    assert set(fields) == {"title", "createdAt", "shapes", "mainShape", "by", "tags", "note"}
    assert fields["mainShape"]["kind"]["tagged"]["tag"] == "type"
    for d in [drawing(), drawing(note="n", shapes=[])]:
        text = d.model_dump_json(by_alias=True)
        assert identity.digest(q.parse_json(text)) == q.digest(d)


def test_root_models_and_model_serializers() -> None:
    class Ids(RootModel[list[int]]):
        pass

    class Money(BaseModel):
        cents: int

        @model_serializer
        def ser(self) -> str:
            return f"{self.cents / 100:.2f}"

    assert q.digest(Ids([1, 2])) == q.digest([1, 2])
    assert q.Identity(Ids).to_json() == {"root": {"list": "integer"}}
    assert q.digest(Money(cents=150)) == q.digest("1.50")
    assert q.digest([Money(cents=150)]) == q.digest(["1.50"])
    with pytest.raises(TypeError):
        q.digestible(Money)


def test_pydantic_dataclasses() -> None:
    @pydantic.dataclasses.dataclass
    class P:
        x: int
        y: Annotated[int, q.Skip()] = 0

    assert q.digest(P(1, 2)) == q.digest({"x": 1})
