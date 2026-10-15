"""Digests of pydantic models, following each model's own configuration.

pip install 'quiddity-digest[pydantic]'
python examples/pydantic_models.py
"""

from datetime import UTC, datetime
from typing import Annotated, Literal

from pydantic import BaseModel, ConfigDict, Field, field_serializer
from pydantic.alias_generators import to_camel

import quiddity as q


class Circle(BaseModel):
    type: Literal["circle"] = "circle"
    radius: float


class Square(BaseModel):
    type: Literal["square"] = "square"
    side: float


@q.digestible(domain="example.drawing")
class Drawing(BaseModel):
    # Fields are digested under their aliases: createdAt, mainShape, by.
    model_config = ConfigDict(alias_generator=to_camel)

    title: str
    created_at: Annotated[datetime, q.Quantize(1_000_000_000)]
    main_shape: Circle | Square = Field(discriminator="type")
    author: str = Field(serialization_alias="by")
    # Excluded from the model's JSON form, and so from its digest.
    session: str = Field(exclude=True)
    tags: Annotated[list[str], q.AsSet()] = []

    # A field with a custom serializer is digested as the serializer writes it.
    @field_serializer("title")
    def strip(self, title: str) -> str:
        return title.strip()


drawing = Drawing(
    title="  Plan  ",
    createdAt=datetime(2026, 10, 15, 9, 30, 0, 250_000, tzinfo=UTC),
    mainShape=Circle(radius=2),
    author="ada",
    session="s-1",
    tags=["a", "b"],
)
digest = q.digest(drawing)
print(f"digest:                  {digest}")

other = drawing.model_copy(update={"session": "s-2", "tags": ["b", "a"]})
print(f"unchanged:               {q.digest(other) == digest}")

# The model's identity, for other languages, applies to its JSON form.
identity = q.identity_of(Drawing)
text = drawing.model_dump_json(by_alias=True)
print(f"JSON form:               {text}")
print(f"JSON form digests alike: {identity.digest(q.parse_json(text)) == digest}")
