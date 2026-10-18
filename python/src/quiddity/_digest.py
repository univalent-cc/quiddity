"""Digests (spec 4)."""

from __future__ import annotations

import builtins
import hashlib
import re
from typing import Final

from ._model import wtf8

_VALUE: Final = b"quiddity/1/value"
_DOMAIN: Final = b"quiddity/1/domain"
_PREFIXES: Final = {32: "sha256:", 16: "sha256-128:", 8: "sha256-64:"}
_TEXT: Final = re.compile(r"(sha256|sha256-128|sha256-64):([0-9a-fA-F]+)\Z")


class Digest:
    """A digest: the 256-bit SHA-256 digest of a value's encoding (spec 4),
    or one of its shortened forms of 128 or 64 bits (spec 4.3).

    A digest is immutable, hashable and comparable. ``str(digest)`` is its
    text form (spec 4.4), which :meth:`parse` reads back.

    >>> import quiddity
    >>> d = quiddity.digest({"title": "Moby Dick"})
    >>> str(d)
    'sha256:2b9a626f5983b54b439c22562ed18f65a0331e34e4788d0471706330ed9cc541'
    >>> str(d.micro())
    'sha256-64:2b9a626f5983b54b'
    >>> Digest.parse(str(d)) == d
    True
    """

    __slots__ = ("_bytes",)

    _bytes: bytes

    def __init__(self, digest: bytes) -> None:
        if len(digest) not in _PREFIXES:
            raise ValueError(f"a digest is 32, 16 or 8 bytes, not {len(digest)}")
        object.__setattr__(self, "_bytes", bytes(digest))

    @classmethod
    def of_encoding(cls, encoding: bytes) -> Digest:
        """The structural digest S(v) of a value whose encoding is
        ``encoding`` (spec 4.1)."""
        return cls(hashlib.sha256(_VALUE + encoding).digest())

    @classmethod
    def parse(cls, text: str) -> Digest:
        """Reads a digest's text form (spec 4.4), raising ``ValueError`` if
        ``text`` is not one. Hex digits may be upper case."""
        m = _TEXT.match(text)
        if not m:
            raise ValueError(f"not a digest: {text!r}")
        size = {"sha256": 32, "sha256-128": 16, "sha256-64": 8}[m.group(1)]
        hexits = m.group(2)
        if len(hexits) != size * 2:
            raise ValueError(f"not a digest: {text!r}")
        return cls(bytes.fromhex(hexits))

    @property
    def bytes(self) -> bytes:
        """The digest's bytes: 32, 16 or 8 of them."""
        return self._bytes

    @property
    def bits(self) -> int:
        """The digest's size in bits: 256, 128 or 64."""
        return len(self._bytes) * 8

    def mini(self) -> Digest:
        """The 128-bit form: the first 16 bytes (spec 4.3)."""
        return self._shorten(16)

    def micro(self) -> Digest:
        """The 64-bit form: the first 8 bytes (spec 4.3), short enough for
        an ETag."""
        return self._shorten(8)

    def _shorten(self, size: int) -> Digest:
        if len(self._bytes) < size:
            raise ValueError(f"a {self.bits}-bit digest cannot be shortened to {size * 8} bits")
        return Digest(self._bytes[:size])

    def in_domain(self, label: str | builtins.bytes) -> Digest:
        """This digest placed in the domain ``label`` (spec 4.2). Digests in
        different domains never collide, so values of different types with
        the same fields have different digests in their own domains."""
        if len(self._bytes) != 32:
            raise ValueError("only a 256-bit digest can be placed in a domain")
        lb = wtf8(label) if isinstance(label, str) else bytes(label)
        if not lb:
            raise ValueError("a domain label is not empty")
        h = hashlib.sha256(_DOMAIN + lb + len(lb).to_bytes(8, "big") + self._bytes)
        return Digest(h.digest())

    def __str__(self) -> str:
        return _PREFIXES[len(self._bytes)] + self._bytes.hex()

    def __repr__(self) -> str:
        return f"Digest({str(self)!r})"

    def __eq__(self, other: object) -> bool:
        return isinstance(other, Digest) and other._bytes == self._bytes

    def __lt__(self, other: Digest) -> bool:
        if not isinstance(other, Digest):
            return NotImplemented
        return (len(self._bytes), self._bytes) < (len(other._bytes), other._bytes)

    def __hash__(self) -> int:
        return hash(self._bytes)

    def __setattr__(self, name: str, value: object) -> None:
        raise AttributeError("a Digest is immutable")

    def __reduce__(self) -> tuple[type[Digest], tuple[builtins.bytes]]:
        return (Digest, (self._bytes,))
