// The examples run, and print what their comments say.

import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { describe, test } from "node:test";

function run(name: string): string[] {
  const file = new URL(`../examples/${name}`, import.meta.url);
  return execFileSync(process.execPath, [file.pathname], { encoding: "utf8" }).trimEnd().split("\n");
}

describe("examples", () => {
  test("books.ts", () => {
    const [same, json, digest] = run("books.ts");
    assert.equal(same, "true");
    assert.match(json!, /^\{"root":\{"object":/);
    // The identity's digest, which the Python package computes for the same identity.
    assert.equal(digest, "sha256:7cb91f1313e1951fb394da9f289a377d0fdc9da86ac39105d9b654a2bd6f8894");
  });

  test("json_identity.ts", () => {
    // The digest the Python package gives the same document under the same identity file.
    assert.deepEqual(run("json_identity.ts"), [
      "true",
      "sha256:bafbd244db073cbf4b1ed7d11b513b993ae7757500f19a04dadc054f3df462fe",
      "true",
    ]);
  });
});
