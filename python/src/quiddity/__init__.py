"""Stable, configurable digests that capture the essence of values.

A digest is the same in every language that implements quiddity, on every
platform, and in every release. Which parts of a value count is an
*identity*: a declared class's fields, with options in ``Annotated``
markers, or an :class:`Identity` as data, which applies to any value.

>>> import quiddity
>>> str(quiddity.digest({"prompt": "Summarize the report"}))
'sha256:d438cd022c59365720a55a65784c1d62922c74d2870c86899570f9cbf40c6757'

The encoding is specified in ``QUIDDITY_SPEC.md``, in the repository at
https://github.com/univalent-cc/quiddity.
"""

from typing import Any

from . import _declared, _value
from ._declared import digestible
from ._digest import Digest
from ._errors import InvalidIdentity, QuiddityError
from ._identity import Identity, identity_of
from ._json import parse_json
from ._markers import (
    AsSet,
    Decimals,
    Flatten,
    Hex,
    Nested,
    OmitEmpty,
    OmitNone,
    Quantize,
    Rename,
    Skip,
    Tagged,
    Unit,
)
from ._model import encode as _encode

__all__ = [
    "AsSet",
    "Decimals",
    "Digest",
    "Flatten",
    "Hex",
    "Identity",
    "InvalidIdentity",
    "Nested",
    "OmitEmpty",
    "OmitNone",
    "Quantize",
    "QuiddityError",
    "Rename",
    "Skip",
    "Tagged",
    "Unit",
    "digest",
    "digestible",
    "encoding",
    "identity_of",
    "parse_json",
]

__version__ = "0.1.0"

# The public classes are documented, and shown in tracebacks, as quiddity's.
for _cls in (Digest, Identity, InvalidIdentity, QuiddityError):
    _cls.__module__ = "quiddity"
del _cls


def digest(value: Any, identity: str | Identity = "primary") -> Digest:
    """The digest of ``value``, under an identity.

    ``identity`` is the name of one of a declared class's identities
    (``"primary"`` by default), or an :class:`Identity`, which applies to
    any value. Raises :class:`QuiddityError` if the value has no digest.

    Values are read as their JSON forms would be, except where the data
    model says more than JSON: a ``datetime`` is an instant, the same in
    any time zone; a ``set`` is a set, whose order does not count; bytes
    are bytes; and dict keys keep their types.

    >>> a = digest({"b": 1, "a": [1.0, "x"]})
    >>> a == digest({"a": [1, "x"], "b": 1})
    True
    >>> digest(object())
    Traceback (most recent call last):
    ...
    quiddity.QuiddityError: a object has no data-model value (at $)
    """
    if isinstance(identity, Identity):
        return identity.digest(value)
    model, domain = _read(value, identity)
    d = Digest.of_encoding(_encode(model))
    return d.in_domain(domain) if domain is not None else d


def encoding(value: Any, identity: str | Identity = "primary") -> bytes:
    """The encoding of ``value`` (spec section 3), the bytes its digest
    hashes, under an identity as :func:`digest` takes it.

    >>> encoding([True]).hex()
    '03000000000000000130'
    """
    if isinstance(identity, Identity):
        return _encode(identity.read(value))
    return _encode(_read(value, identity)[0])


def _read(value: Any, identity: str) -> tuple[Any, str | None]:
    if not isinstance(identity, str):
        raise TypeError(f"an identity is a name or an Identity, not {identity!r}")
    v = _declared.read(value, identity, "$")
    if v is not _declared.NOT_DECLARED:
        return v, _declared.domain(value)
    if identity != _declared.PRIMARY:
        raise QuiddityError(
            f"a {type(value).__qualname__} is not a declared class, so it has no identity "
            f"{identity!r}"
        )
    return _value.from_python(value), None
