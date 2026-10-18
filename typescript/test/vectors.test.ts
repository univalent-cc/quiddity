// The language-neutral test vectors (test-vectors/quiddity-v1.json), which
// every implementation must reproduce.

import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
import { describe, test } from "node:test";

import * as q from "../dist/index.js";
import { type RawJson, type RawNumber, jsonText, parseRawJson } from "../dist/json.js";

const PATH = new URL("../../test-vectors/quiddity-v1.json", import.meta.url);

type Case = Record<string, RawJson>;

// Read with each number's text kept: JSON.parse would round large integers.
const VECTORS = existsSync(PATH) ? (parseRawJson(readFileSync(PATH, "utf8")) as Record<string, RawJson>) : undefined;

function cases(section: string): Case[] {
  return (VECTORS?.[section] ?? []) as Case[];
}

/** The vectors' typed notation as data-model values. */
function value(v: RawJson): q.Value {
  const [[tag, x]] = Object.entries(v as Record<string, RawJson>) as [[string, RawJson]];
  switch (tag) {
    case "null":
      return null;
    case "bool":
      return x as boolean;
    case "int":
      return BigInt(x as string);
    case "float": {
      const view = new DataView(new ArrayBuffer(8));
      view.setBigUint64(0, BigInt(`0x${x as string}`));
      return view.getFloat64(0);
    }
    case "string":
      return x as string;
    case "code_points":
      return String.fromCodePoint(...(x as RawNumber[]).map((n) => Number(n.text)));
    case "bytes":
      return Uint8Array.from((x as string).match(/../g) ?? [], (h) => parseInt(h, 16));
    case "list":
      return (x as RawJson[]).map(value);
    case "set":
      return new q.ValueSet((x as RawJson[]).map(value));
    case "map":
      return new q.ValueMap((x as RawJson[][]).map(([k, i]) => [value(k!), value(i!)] as const));
  }
  throw new Error(`unknown tag ${tag}`);
}

function hex(bytes: Uint8Array): string {
  return Array.from(bytes, (b) => b.toString(16).padStart(2, "0")).join("");
}

function checkDigests(d: q.Digest, c: Case): void {
  assert.equal(String(d), c["digest"]);
  assert.equal(String(d.mini()), c["digest128"]);
  assert.equal(String(d.micro()), c["digest64"]);
}

describe("test vectors", { skip: VECTORS === undefined ? "the test vectors are not present" : false }, () => {
  test("values", () => {
    for (const c of cases("values")) {
      const v = value(c["value"]!);
      assert.equal(hex(q.encoding(v)), c["encoding"], c["name"] as string);
      checkDigests(q.digest(v), c);
    }
  });

  test("domains", () => {
    for (const c of cases("domains")) {
      const structural = q.digest(value(c["value"]!));
      assert.equal(String(structural), c["structural"]);
      checkDigests(structural.inDomain(c["label"] as string), c);
    }
  });

  test("json", () => {
    for (const c of cases("json")) checkDigests(q.digestJson(c["text"] as string), c);
  });

  test("identities", () => {
    for (const c of cases("identities")) {
      const name = c["name"] as string;
      const identity = q.Identity.fromJson(jsonText(c["identity"]));
      assert.equal(String(q.digest(identity)), c["identity_digest"], name);
      assert.ok(q.Identity.fromJson(identity.toJson()).equals(identity), name);
      assert.ok(q.Identity.fromJson(identity.toJsonText()).equals(identity), name);
      for (const s of c["cases"] as Case[]) {
        const source = q.parseJson(jsonText(s["source"]));
        const where = `${name}: ${jsonText(s["source"])}`;
        if (s["error"] === true) {
          assert.throws(() => identity.digest(source), q.QuiddityError, where);
          continue;
        }
        assert.equal(hex(identity.encoding(source)), s["encoding"], where);
        checkDigests(identity.digest(source), s);
      }
    }
  });

  test("invalid identities", () => {
    for (const c of cases("invalid_identities")) {
      assert.throws(() => q.Identity.fromJson(jsonText(c["identity"])), q.InvalidIdentity, c["name"] as string);
    }
  });

  test("same identity", () => {
    for (const c of cases("same_identity")) {
      for (const form of c["identities"] as RawJson[]) {
        assert.equal(String(q.digest(q.Identity.fromJson(jsonText(form)))), c["identity_digest"], c["name"] as string);
      }
    }
  });

  test("identity of identities", () => {
    const c = VECTORS!["identity_of_identities"] as Case;
    const meta = q.Identity.fromJson(jsonText(c["identity"]));
    assert.equal(String(q.digest(meta)), c["identity_digest"]);
    // Applied to itself, as an identity, it gives the same digest.
    assert.equal(String(meta.digest(meta.toJson())), c["identity_digest"]);
    for (const i of cases("identities")) {
      const identity = q.Identity.fromJson(jsonText(i["identity"]));
      assert.ok(meta.digest(identity.toJson()).equals(q.digest(identity)));
    }
  });

  test("text forms", () => {
    for (const c of cases("text_forms")) {
      const text = c["text"] as string;
      if (c["valid"] === true) {
        assert.equal(String(q.Digest.parse(text)).toLowerCase(), text.toLowerCase());
      } else {
        assert.throws(() => q.Digest.parse(text), SyntaxError, text);
      }
    }
  });
});
