"""The examples run, and print the digests the Rust crate's and the Elixir
package's examples print for the same values."""

import pathlib
import subprocess
import sys

import pytest

EXAMPLES = pathlib.Path(__file__).resolve().parents[1] / "examples"


def run(*args: str) -> str:
    return subprocess.run(
        [sys.executable, *args], cwd=EXAMPLES.parent, capture_output=True, text=True, check=True
    ).stdout


def test_values() -> None:
    out = run("examples/values.py")
    # As the Elixir package's examples/values.exs.
    assert "sha256:fad911ecab7d6c7c681fea6c604041936ab6b853a19522db118a327fac4be56a" in out
    assert "False" not in out.replace("lists:          False", "")


def test_dynamic_identity() -> None:
    out = run("examples/dynamic_identity.py")
    # As the Rust crate's examples/dynamic_identity.rs and the Elixir
    # package's examples/dynamic_identity.exs.
    assert "sha256:cadb496f7865e4fea62732c25f187c400b16e707a5f32721d56253dd1ef0d867" in out
    assert "sha256:bfe610c85a00edf4f1839547de54986b056346540c734d9f31de8eb6eee4a3e0" in out
    assert "sha256:0c0c0a3d2307dfd21e5ae5acbec47e4e9228d0e54fbafd0061f2060c153dde8a" in out
    assert "False" not in out


def test_declared() -> None:
    out = run("examples/declared.py")
    # As the Elixir package's examples/structs.exs.
    assert "sha256:0bacbce1fa51172b137f4fec1ac121fc334723db66439d61520f9868e7c6579c" in out
    assert "sha256:efe02eb970ff75620602913f05432d2933ce33f7b95546963e58f8b37bdc43f0" in out
    assert "False" not in out


def test_digest_json() -> None:
    data = "examples/data/"
    out = run("examples/digest_json.py", data + "book.identity.json", data + "book.json")
    # As the Rust crate's and the Elixir package's examples/digest_json.
    assert "sha256:6fa1bb74499c51d4f40167f9e96c1b71560eb6179a402a552a797b385db1a2d4" in out
    assert "sha256:5247bc0c92298aa6484aa98f4d1537ae60e14a3cb72f3445e5d2592294f410be" in out


@pytest.mark.parametrize("name", ["pydantic_models.py"])
def test_others(name: str) -> None:
    if (EXAMPLES / name).exists():
        assert "False" not in run(f"examples/{name}")
