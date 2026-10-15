"""Digests JSON files under an identity read from a JSON file.

    python examples/digest_json.py examples/data/task.identity.json \\
        examples/data/task.json examples/data/task-reordered.json

With no value files, reads one JSON value from standard input.
"""

import pathlib
import sys

import quiddity as q


def main(args: list[str]) -> int:
    if not args:
        print(
            "usage: python examples/digest_json.py <identity.json> [value.json …]", file=sys.stderr
        )
        return 1
    identity = q.Identity.from_json(pathlib.Path(args[0]).read_text())
    print(f"{q.digest(identity)}  (identity)")
    inputs = [(p, pathlib.Path(p).read_text()) for p in args[1:]] or [("-", sys.stdin.read())]
    for name, text in inputs:
        try:
            print(f"{identity.digest(q.parse_json(text))}  {name}")
        except q.QuiddityError as e:
            print(f"{name}: {e}", file=sys.stderr)
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
