"""Quantization (spec 5.6): a value rounded down to a grid, exactly."""

from __future__ import annotations

import math
from fractions import Fraction
from typing import Any

from ._errors import QuiddityError
from ._model import Duration, Instant, normalize


def quantize(v: Any, step: int) -> Any:
    """``v`` rounded down to a multiple of ``step``: an integer, an instant
    or a duration (in nanoseconds). None stays None."""
    t = type(v)
    if v is None:
        return None
    if t is int:
        return (v // step) * step
    if t is Instant or t is Duration:
        return t((v.ns // step) * step)
    raise QuiddityError(f"quantize applies to integers, instants and durations, not {_what(v)}")


def decimals(v: Any, d: int) -> Any:
    """``v`` rounded down to a multiple of 10^-d. A float is taken as the
    shortest decimal that reads back as it (its ``repr``), so ``2.67`` stays
    ``2.67``. The result is an integer when its value is one."""
    t = type(v)
    if v is None:
        return None
    if t is float and not math.isfinite(v):
        return v
    if t is int or t is float:
        exact = Fraction(repr(v)) if t is float else Fraction(v)
        scale = Fraction(10) ** d
        r = Fraction(math.floor(exact * scale)) / scale
        return int(r) if r.denominator == 1 else normalize(float(r))
    raise QuiddityError(f"decimals applies to numbers, not {_what(v)}")


def _what(v: Any) -> str:
    return {str: "a string", bytes: "a string", list: "a list", bool: "a boolean"}.get(
        type(v), type(v).__name__.lower()
    )
