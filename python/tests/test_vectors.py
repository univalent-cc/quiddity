"""The language-neutral test vectors (test-vectors/quiddity-v1.json), which
every implementation must reproduce."""

from __future__ import annotations

import json
import pathlib
import struct
from typing import Any

import pytest

import quiddity as q
from quiddity._model import Map, Set

PATH = pathlib.Path(__file__).resolve().parents[2] / "test-vectors" / "quiddity-v1.json"

if not PATH.exists():
    pytest.skip("the test vectors are not present", allow_module_level=True)

VECTORS = json.loads(PATH.read_text())


def value(v: dict[str, Any]) -> Any:
    """The vectors' typed notation as data-model values."""
    ((tag, x),) = v.items()
    if tag == "null":
        return None
    if tag == "bool":
        return x
    if tag == "int":
        return int(x)
    if tag == "float":
        return struct.unpack(">d", bytes.fromhex(x))[0]
    if tag == "string":
        return x
    if tag == "code_points":
        return "".join(map(chr, x))
    if tag == "bytes":
        return bytes.fromhex(x)
    if tag == "list":
        return [value(i) for i in x]
    if tag == "set":
        return Set([value(i) for i in x])
    if tag == "map":
        return Map([(value(k), value(i)) for k, i in x])
    raise AssertionError(tag)


def check_digests(d: q.Digest, case: dict[str, Any]) -> None:
    assert str(d) == case["digest"]
    assert str(d.mini()) == case["digest128"]
    assert str(d.micro()) == case["digest64"]


@pytest.mark.parametrize("case", VECTORS["values"], ids=lambda c: c["name"])
def test_values(case: dict[str, Any]) -> None:
    v = value(case["value"])
    assert q.encoding(v).hex() == case["encoding"]
    check_digests(q.digest(v), case)


@pytest.mark.parametrize("case", VECTORS["domains"], ids=lambda c: c["name"])
def test_domains(case: dict[str, Any]) -> None:
    structural = q.digest(value(case["value"]))
    assert str(structural) == case["structural"]
    check_digests(structural.in_domain(case["label"]), case)


@pytest.mark.parametrize("case", VECTORS["json"], ids=lambda c: c["text"][:40])
def test_json(case: dict[str, Any]) -> None:
    check_digests(q.digest(q.parse_json(case["text"])), case)


@pytest.mark.parametrize("case", VECTORS["identities"], ids=lambda c: c["name"])
def test_identities(case: dict[str, Any]) -> None:
    identity = q.Identity.from_json(case["identity"])
    assert str(q.digest(identity)) == case["identity_digest"]
    assert q.Identity.from_json(identity.to_json()) == identity
    assert q.Identity.from_json(json.dumps(identity.to_json())) == identity
    for c in case["cases"]:
        source = q.parse_json(json.dumps(c["source"]))
        if c.get("error"):
            with pytest.raises(q.QuiddityError):
                identity.digest(source)
            continue
        assert q.encoding(source, identity).hex() == c["encoding"], c["source"]
        check_digests(identity.digest(source), c)


@pytest.mark.parametrize("case", VECTORS["invalid_identities"], ids=lambda c: c["name"])
def test_invalid_identities(case: dict[str, Any]) -> None:
    with pytest.raises(q.InvalidIdentity):
        q.Identity.from_json(json.dumps(case["identity"]))


@pytest.mark.parametrize("case", VECTORS["same_identity"], ids=lambda c: c["name"])
def test_same_identity(case: dict[str, Any]) -> None:
    for form in case["identities"]:
        assert str(q.digest(q.Identity.from_json(form))) == case["identity_digest"]


def test_identity_of_identities() -> None:
    case = VECTORS["identity_of_identities"]
    meta = q.Identity.from_json(case["identity"])
    assert str(q.digest(meta)) == case["identity_digest"]
    # Applied to itself, as an identity, it gives the same digest.
    assert str(meta.digest(meta.to_json())) == case["identity_digest"]
    for c in VECTORS["identities"]:
        identity = q.Identity.from_json(c["identity"])
        assert meta.digest(identity.to_json()) == q.digest(identity)


@pytest.mark.parametrize("case", VECTORS["text_forms"], ids=lambda c: c["text"])
def test_text_forms(case: dict[str, Any]) -> None:
    if case["valid"]:
        assert str(q.Digest.parse(case["text"])).lower() == case["text"].lower()
    else:
        with pytest.raises(ValueError):
            q.Digest.parse(case["text"])
