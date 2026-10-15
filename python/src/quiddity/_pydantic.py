"""pydantic models as declared classes (``history/PYTHON.md``, PY8): a model is
digested as its configuration says, field by field.

This module is imported only for a class that is a pydantic model, so
pydantic is never imported by quiddity itself."""

from __future__ import annotations

from typing import Any

import pydantic

from ._declared import Source
from ._markers import Tagged


def sources(cls: type[pydantic.BaseModel]) -> tuple[Source, ...]:
    """A model's fields: excluded fields left out, each under the name its
    JSON form gives it (its serialization alias, its alias, or the alias
    generator's), with its ``Annotated`` metadata and its custom serializer."""
    serializers = _field_serializers(cls)
    out = []
    for name, info in cls.model_fields.items():
        if info.exclude is True:
            continue
        key = info.serialization_alias or info.alias or name
        metadata: tuple[Any, ...] = tuple(info.metadata)
        if isinstance(info.discriminator, str):
            # A discriminated union: an internally tagged union (spec 5.8).
            metadata = (*metadata, Tagged(info.discriminator))
        out.append(
            Source(
                attr=name,
                key=key,
                hint=info.annotation,
                metadata=metadata,
                serializer=serializers.get(name),
                renamed=key != name,
            )
        )
    return tuple(out)


def _field_serializers(cls: type[pydantic.BaseModel]) -> dict[str, Any]:
    """For each field with a ``field_serializer``, a function giving its
    serialized value, through pydantic's own serializer for the field."""
    out: dict[str, Any] = {}
    for decorator in cls.__pydantic_decorators__.field_serializers.values():
        info = decorator.info
        mode = "json" if info.when_used in ("json", "json-unless-none") else "python"
        for field in info.fields:
            out[field] = _serialize(field, mode)
    return out


def _serialize(field: str, mode: str) -> Any:
    def serialize(model: pydantic.BaseModel, _raw: Any) -> Any:
        dumped = model.__pydantic_serializer__.to_python(model, include={field}, mode=mode)
        return dumped[field]

    return serialize


def has_model_serializer(cls: type) -> bool:
    """Whether a model writes itself with a ``model_serializer``, or is a
    ``RootModel``: its digest is then that of its serialized form."""
    if issubclass(cls, pydantic.RootModel):
        return True
    decorators = getattr(cls, "__pydantic_decorators__", None)
    return bool(decorators and decorators.model_serializers)


def serialized(model: pydantic.BaseModel) -> Any:
    """A model as its ``model_serializer`` writes it, in Python mode."""
    if isinstance(model, pydantic.RootModel):
        return model.root
    return model.model_dump(mode="python", by_alias=True)
