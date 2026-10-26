"""The conversions of spec 5.5: values written as text, read as the kind
declares. Each function raises ``QuiddityError`` (without a path) for text
it cannot read."""

from __future__ import annotations

import base64
import re
from fractions import Fraction
from typing import Final

from ._errors import QuiddityError
from ._model import normalize, wtf8

_MIN, _MAX = -(2**63), 2**64 - 1
# Decimal text with more digits, or a larger exponent, is refused, so that
# reading it cannot take unbounded time or memory.
MAX_DIGITS: Final = 4096

_WS: Final = " \t\n\r"
_DECIMAL: Final = re.compile(r"([+-]?)(0|[1-9][0-9]*)(?:\.([0-9]+))?(?:[eE]([+-]?[0-9]+))?\Z")
_JSON_INT: Final = re.compile(r"-?(?:0|[1-9][0-9]*)\Z")
_BOOLS: Final = {
    "true": True, "t": True, "yes": True, "y": True, "on": True, "1": True,
    "false": False, "f": False, "no": False, "n": False, "off": False, "0": False,
}  # fmt: skip
_NUMBER_TOKENS: Final = {
    "nan": float("nan"), "+nan": float("nan"), "-nan": float("nan"),
    "inf": float("inf"), "+inf": float("inf"), "infinity": float("inf"),
    "+infinity": float("inf"), "∞": float("inf"), "+∞": float("inf"),
    "-inf": float("-inf"), "-infinity": float("-inf"), "-∞": float("-inf"),
}  # fmt: skip
_UUID: Final = re.compile(r"[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}\Z")
_UUID_SIMPLE: Final = re.compile(r"[0-9a-f]{32}\Z")
_RFC3339: Final = re.compile(
    r"([0-9]{4})-([0-9]{2})-([0-9]{2})[Tt ]([0-9]{2}):([0-9]{2}):([0-9]{2})(?:\.([0-9]{1,9}))?"
    r"(?:([Zz])|([+-])([0-9]{2})(?::?([0-9]{2}))?)?\Z"
)
_ISO_DURATION: Final = re.compile(
    r"([+-])?[Pp](?:([0-9]+)[Ww])?(?:([0-9]+)[Dd])?"
    r"(?:[Tt](?:([0-9]+)[Hh])?(?:([0-9]+)[Mm])?(?:([0-9]+)(?:\.([0-9]{1,9}))?[Ss])?)?\Z"
)


def trim(s: str) -> str:
    """Spec 5.5 removes ASCII whitespace only, from both ends."""
    return s.strip(_WS)


def boolean(s: str) -> bool:
    word = trim(s).lower()
    if word not in _BOOLS:
        raise QuiddityError(f"not a boolean: {s!r}")
    return _BOOLS[word]


def decimal(text: str) -> Fraction | None:
    """Decimal text as an exact fraction, or None if it is not decimal text."""
    m = _DECIMAL.match(text)
    if not m:
        return None
    sign, whole, frac, exp = m.groups()
    digits = whole + (frac or "")
    e = int(exp) if exp is not None and len(exp) <= 12 else (0 if exp is None else 10**13)
    if len(digits) > MAX_DIGITS or abs(e) > MAX_DIGITS:
        raise QuiddityError(f"out of range: {text[:40]!r}")
    e -= len(frac or "")
    n = int(digits) * (-1 if sign == "-" else 1)
    return Fraction(n * 10**e) if e >= 0 else Fraction(n, 10**-e)


def integer(s: str) -> int:
    f = decimal(trim(s))
    if f is None or f.denominator != 1:
        raise QuiddityError(f"not an integer: {s!r}")
    return int(f)


def number(s: str) -> int | float:
    text = trim(s)
    token = text.lower().replace("\u2212", "-")  # a minus sign
    if token in _NUMBER_TOKENS:
        return _NUMBER_TOKENS[token]
    if not _DECIMAL.match(text):
        raise QuiddityError(f"not a number: {s!r}")
    return json_number(text)


def json_number(text: str) -> int | float:
    """The value spec 2.7 gives decimal text as a JSON number."""
    body = text[1:] if text.startswith("+") else text
    if len(body) <= 21 and _JSON_INT.match(body):
        n = int(body)
        if _MIN <= n <= _MAX:
            return n
    return normalize(float(body))


def uuid(s: str) -> str:
    text = trim(s).lower()
    if text.startswith("{") and text.endswith("}"):
        text = text[1:-1]
    elif text.startswith("urn:uuid:"):
        text = text[9:]
        if not _UUID.match(text):
            raise QuiddityError(f"not a UUID: {s!r}")
    if _UUID_SIMPLE.match(text):
        text = "-".join((text[:8], text[8:12], text[12:16], text[16:20], text[20:]))
    if not _UUID.match(text):
        raise QuiddityError(f"not a UUID: {s!r}")
    return text


def hex_bytes(s: str) -> bytes:
    text = trim(s)
    if not re.fullmatch(r"(?:[0-9a-fA-F]{2})*", text):
        raise QuiddityError(f"not hex: {s!r}")
    return bytes.fromhex(text)


def base64_bytes(s: str) -> bytes:
    """Standard or URL-safe base64, padded or not, but not mixed."""
    text = trim(s)
    body = text.rstrip("=")
    chars = set(body)
    if chars & set("+/") and chars & set("-_"):
        raise QuiddityError("base64 mixes the standard and URL-safe alphabets")
    padding = len(text) - len(body)
    if padding and (len(text) % 4 or padding > 2):
        raise QuiddityError("incorrect base64 padding")
    if len(body) % 4 == 1:
        raise QuiddityError("not base64")
    body = body.translate(str.maketrans("-_", "+/"))
    try:
        return base64.b64decode(body + "=" * (-len(body) % 4), validate=True)
    except ValueError as e:
        raise QuiddityError(f"not base64: {e}") from None


def windows_path(units: list[int]) -> bytes:
    """A Windows path's UTF-16 code units as WTF-8 bytes."""
    return wtf8("".join(map(chr, units)))


def _days_from_civil(y: int, m: int, d: int) -> int:
    """Days since 1970-01-01 in the proleptic Gregorian calendar."""
    y -= m <= 2
    era = y // 400
    yoe = y - era * 400
    doy = (153 * (m + (-3 if m > 2 else 9)) + 2) // 5 + d - 1
    doe = yoe * 365 + yoe // 4 - yoe // 100 + doy
    return era * 146097 + doe - 719468


def _days_in_month(y: int, m: int) -> int:
    if m == 2:
        return 29 if (y % 4 == 0 and y % 100 != 0) or y % 400 == 0 else 28
    return 30 if m in (4, 6, 9, 11) else 31


def instant(text: str) -> int:
    """RFC 3339 text, UTC without an offset, as nanoseconds since the Unix epoch."""
    m = _RFC3339.match(text)
    if not m:
        raise QuiddityError(f"not an RFC 3339 date-time: {text!r}")
    y, mo, d, h, mi, s = (int(g) for g in m.groups()[:6])
    frac = m.group(7) or ""
    # A second of 60 is a leap second, counted as the next minute's first (spec 2.6).
    if not (
        1 <= mo <= 12 and 1 <= d <= _days_in_month(y, mo) and h <= 23 and mi <= 59 and s <= 60
    ):
        raise QuiddityError(f"date or time out of range: {text!r}")
    offset = 0
    if m.group(9):
        oh, om = int(m.group(10)), int(m.group(11) or 0)
        if oh > 23 or om > 59:
            raise QuiddityError(f"offset out of range: {text!r}")
        offset = (oh * 3600 + om * 60) * (-1 if m.group(9) == "-" else 1)
    secs = _days_from_civil(y, mo, d) * 86400 + h * 3600 + mi * 60 + s - offset
    return secs * 10**9 + int(frac.ljust(9, "0") or 0)


def duration(text: str) -> int:
    """ISO 8601 duration text without years or months, as nanoseconds."""
    m = _ISO_DURATION.match(text)
    if not m or not any(m.groups()[1:6]) or text[-1:] in ("T", "t"):
        raise QuiddityError(f"not an ISO 8601 duration without years or months: {text!r}")
    sign, w, d, h, mi, s, frac = m.groups()
    if any(g is not None and len(g) > 30 for g in (w, d, h, mi, s)):
        raise QuiddityError(f"out of range: {text[:40]!r}")
    secs = (
        (int(w or 0) * 7 + int(d or 0)) * 86400
        + int(h or 0) * 3600
        + int(mi or 0) * 60
        + int(s or 0)
    )
    ns = secs * 10**9 + int((frac or "").ljust(9, "0") or 0)
    return -ns if sign == "-" else ns
