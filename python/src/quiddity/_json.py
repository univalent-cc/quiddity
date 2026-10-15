"""JSON text read as spec 2.7 reads it."""

from __future__ import annotations

import json
from typing import Any

from ._errors import QuiddityError
from ._model import normalize

_MIN, _MAX = -(2**63), 2**64 - 1


def parse_json(text: str | bytes | bytearray) -> Any:
    """JSON text as Python values, with numbers as spec 2.7 reads them, so
    that a JSON document has the same digest in every language: an integer
    from -2^63 to 2^64 - 1 is that integer, any other number is the nearest
    binary64 value, and ``1.0`` and ``1e0`` are the integer 1. ``json.loads``
    differs only for integers beyond 64 bits, which it keeps exact.

    An object with a repeated member name, and ``NaN`` or ``Infinity``
    (which are not JSON), raise ``QuiddityError``.

    >>> parse_json('{"n": 1.0, "big": 18446744073709551617}')
    {'n': 1, 'big': 18446744073709551616}
    """

    def integer(t: str) -> int | float:
        if len(t) <= 21:
            n = int(t)
            if _MIN <= n <= _MAX:
                return n
        return normalize(float(t))

    def pairs(items: list[tuple[str, Any]]) -> dict[str, Any]:
        out: dict[str, Any] = {}
        for k, v in items:
            if k in out:
                raise QuiddityError(f"the member name {k!r} is repeated")
            out[k] = v
        return out

    def constant(name: str) -> Any:
        raise QuiddityError(f"{name} is not JSON")

    try:
        return json.loads(
            text,
            parse_int=integer,
            parse_float=lambda t: normalize(float(t)),
            parse_constant=constant,
            object_pairs_hook=pairs,
        )
    except QuiddityError:
        raise
    except ValueError as e:
        raise QuiddityError(f"not JSON: {e}") from None
