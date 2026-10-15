"""The exceptions quiddity raises."""

from __future__ import annotations


class QuiddityError(ValueError):
    """A value that has no digest: it, or a part of it, has no data-model
    value, or it does not have the shape an identity describes.

    ``path`` says where, as a JSONPath-like string: ``$`` for the value
    itself, ``$.tags[2]`` for an item inside it.
    """

    path: str
    message: str

    def __init__(self, message: str, path: str = "$") -> None:
        super().__init__(f"{message} (at {path})")
        self.path = path
        self.message = message

    def at(self, prefix: str) -> QuiddityError:
        """The same error, with ``prefix`` prepended to its path."""
        return type(self)(self.message, prefix + self.path[1:])


class InvalidIdentity(QuiddityError):
    """An identity, read or being built, that is not valid (spec 5.9)."""


def nest(prefix: str, e: QuiddityError) -> QuiddityError:
    """``e``, raised inside the value at ``prefix``, with its path made
    relative to the outer value."""
    return e.at(prefix) if e.path.startswith("$") else e
