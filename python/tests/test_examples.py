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
    assert "sha256:63dc8e794f9f19ee506b755d860e040515b295cb3302352f929306ee8d3de9e5" in out
    assert "False" not in out.replace("lists:          False", "")


def test_dynamic_identity() -> None:
    out = run("examples/dynamic_identity.py")
    # As the Rust crate's examples/dynamic_identity.rs and the Elixir
    # package's examples/dynamic_identity.exs.
    assert "sha256:cec09cdc08e108362c09d5a45368b813906922d79bd3554468eeb49728990a67" in out
    assert "sha256:3c59fa972f11f5a88c3b255813fd42540843f9aee3e1fc9280400c3163287a7f" in out
    assert "sha256:4a910ed3f74d8304a617b21a09cbc9be24e148700f4f21425aeddea16bc05632" in out
    assert "False" not in out


def test_declared() -> None:
    out = run("examples/declared.py")
    # As the Elixir package's examples/structs.exs.
    assert "sha256:776889153c44b41dbeabcd8824906fe33c8decc116a599b1fd664f8fe98275bf" in out
    assert "sha256:ad7f4c9c9f977c9cde89a8aab1ba25cfa1974901ef55c262449588566b4d7b8e" in out
    assert "False" not in out


def test_digest_json() -> None:
    data = "examples/data/"
    out = run("examples/digest_json.py", data + "task.identity.json", data + "task.json")
    # As the Rust crate's and the Elixir package's examples/digest_json.
    assert "sha256:cfb886b0964b4fe069ee126b8e5446903c1df186f4264415bc033249b4daf6c1" in out
    assert "sha256:2db88cb12d7f1a88eecbf32a2003510fdff8856fa3742beff1e884bd2eacef0a" in out


@pytest.mark.parametrize("name", ["pydantic_models.py"])
def test_others(name: str) -> None:
    if (EXAMPLES / name).exists():
        assert "False" not in run(f"examples/{name}")
