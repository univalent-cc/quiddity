// JavaScript values in the data model (history/TYPESCRIPT.md, TS3). The
// expected digests were computed by the Python package from the Python
// values that mean the same thing, so they also check the two agree.

import assert from "node:assert/strict";
import { describe, test } from "node:test";

import * as q from "../dist/index.js";

/** A stand-in for a Temporal object: its tag is on its prototype, as Temporal's are. */
function temporal(kind: string, fields: object): object {
  const proto = { [Symbol.toStringTag]: `Temporal.${kind}` };
  return Object.assign(Object.create(proto), fields);
}

function digestText(v: unknown): string {
  return String(q.digest(v));
}

describe("values", () => {
  test("agree with the Python package", () => {
    const expected: [unknown, string][] = [
      [{ title: "Middlemarch", pages: 880 }, "sha256:60f3f09b1b02dac0f2c84302c8d450b78fa1ac6f195961467247717dd7b44945"],
      [new Date("2024-01-01T00:00:00Z"), "sha256:c426156b1c1b8522abc69e7b177407fd6736694c2d124a7b224a29a7e0b714ed"],
      [new Set([2, 1]), "sha256:64406ae97b773d0d58df470287202a49cea075a3bd13fa3a7cd500b329d0bb08"],
      [new Map([[1, "a"], [2, "b"]]), "sha256:dfdbabec8792143b1047cc0b399aee1bf53f277f32b9a56577ff1a732b352063"],
      [new Uint8Array([1, 2]), "sha256:cee203491d38080c54e55b59fc6328def04b53c438cd9075d71e362572b66409"],
      [1.5, "sha256:abee116c081a17dae4b03f4ec6c3339cdb6134726180231494373b1c9ce377ee"],
      [2n ** 64n, "sha256:ea68dd7237027dea9aa364321e866776f344a219dbc0d3eae2f1b03e0553d9a8"],
      [-0, "sha256:1c6e2c7c95d2fd9b5bb439aac67a11822e9f3b28ee12899bf743818a389672c7"],
      ["\ud800", "sha256:09909f4268716c49fb7346940132082a21c5ac92322badeefa123a2ebca9d8af"],
      [
        { shelf: [{ title: "Moby Dick", genres: new Set(["novel", "adventure"]) }], count: 1 },
        "sha256:1f155e118562ffa477dc61daa0199edc45652015f208b2c0be33c76198c26ac1",
      ],
      [new q.Duration(3_600_000_005_000n), "sha256:c0ea30ddaa0a7306b6ae6760347d9f1a7ee65b6d4fa6a37a3c806a7cbbd0162e"],
    ];
    for (const [v, d] of expected) assert.equal(digestText(v), d);
  });

  test("numbers are normalized", () => {
    assert.equal(digestText(1), digestText(1.0));
    assert.equal(digestText(1), digestText(1n));
    assert.equal(digestText(2 ** 64), digestText(2n ** 64n));
    assert.notEqual(digestText(1), digestText("1"));
  });

  test("member order does not count, item order does", () => {
    assert.equal(digestText({ a: 1, b: 2 }), digestText({ b: 2, a: 1 }));
    assert.notEqual(digestText([1, 2]), digestText([2, 1]));
    assert.equal(digestText(new Set([1, 2])), digestText(new Set([2, 1])));
  });

  test("a member whose value is undefined is absent, as in JSON", () => {
    assert.equal(digestText({ title: "Moby Dick", subtitle: undefined }), digestText({ title: "Moby Dick" }));
    assert.throws(() => q.digest(undefined), q.QuiddityError);
    assert.throws(() => q.digest([undefined]), /undefined has no data-model value \(at \$\[0\]\)/);
  });

  test("bytes of every view, text and bytes agree", () => {
    const b = digestText(new Uint8Array([0x61]));
    assert.equal(digestText(new Uint8Array([0x61]).buffer), b);
    assert.equal(digestText(new DataView(new Uint8Array([0x61]).buffer)), b);
    assert.equal(digestText("a"), b); // spec 2.3: text and bytes are one kind
    assert.throws(() => q.digest(new Int16Array([1])), /byte order/);
  });

  test("instants: Date, Instant and Temporal-like objects", () => {
    const d = digestText(new Date(1_704_067_200_000));
    assert.equal(digestText(new q.Instant(1_704_067_200_000_000_000n)), d);
    assert.equal(digestText(q.Instant.fromDate(new Date("2024-01-01T01:00:00+01:00"))), d);
    assert.equal(digestText(temporal("Instant", { epochNanoseconds: 1_704_067_200_000_000_000n })), d);
    assert.throws(() => q.digest(new Date(NaN)), /an invalid Date has no instant/);
    assert.throws(() => q.digest(temporal("PlainDateTime", {})), /has no instant/);
  });

  test("Temporal durations, each field at a fixed length", () => {
    const zero = { years: 0, months: 0, weeks: 0, days: 0, hours: 0, minutes: 0, seconds: 0, milliseconds: 0, microseconds: 0, nanoseconds: 0 };
    const hour = temporal("Duration", { ...zero, hours: 1, microseconds: 5 });
    assert.equal(digestText(hour), digestText(new q.Duration(3_600_000_005_000n)));
    const month = temporal("Duration", { ...zero, months: 1 });
    assert.equal(digestText(month), digestText(new q.Duration(2_629_746_000_000_000n)));
    const year = temporal("Duration", { ...zero, years: 1 });
    assert.equal(digestText(year), digestText(new q.Duration(31_556_952_000_000_000n)));
  });

  test("Map keys keep their types; equal keys are an error", () => {
    assert.notEqual(digestText(new Map([[1, "a"]])), digestText({ 1: "a" }));
    assert.throws(() => q.digest(new Map<unknown, string>([[1, "a"], [1n, "b"]])), /given twice/);
  });

  test("class instances need a static identity", () => {
    class Shelf {
      label = "B4";
    }
    assert.throws(() => q.digest(new Shelf()), /a Shelf has no data-model value; give its class a static \[quiddity.identity\]/);
    assert.throws(() => q.digest({ shelves: [new Shelf()] }), (e: unknown) => e instanceof q.QuiddityError && e.path === "$.shelves[0]");
  });

  test("a value that contains itself is an error", () => {
    const a: unknown[] = [];
    a.push(a);
    assert.throws(() => q.digest(a), /contains itself/);
  });

  test("digests and URLs are their text", () => {
    const d = q.digest(1);
    assert.equal(digestText(d), digestText(String(d)));
    assert.equal(digestText(new URL("https://example.com/a")), digestText("https://example.com/a"));
  });

  test("parseJson keeps integers exact and rejects repeated names", () => {
    assert.deepEqual(q.parseJson('{"n": 18446744073709551615, "m": 5, "x": 1.5}'), { n: 18446744073709551615n, m: 5, x: 1.5 });
    assert.equal(q.parseJson("18446744073709551616"), 18446744073709551616);
    assert.throws(() => q.parseJson('{"a": 1, "a": 2}'), /the member name "a" is repeated/);
    assert.throws(() => q.parseJson("[1,]"), SyntaxError);
    assert.throws(() => q.parseJson("NaN"), SyntaxError);
    const proto = q.parseJson('{"__proto__": 1}') as Record<string, unknown>;
    assert.equal(Object.getPrototypeOf(proto), Object.prototype);
    assert.deepEqual(Object.keys(proto), ["__proto__"]);
  });

  test("digestJson agrees with digest", () => {
    assert.equal(String(q.digestJson('{"pages": 880, "title": "Middlemarch"}')), digestText({ title: "Middlemarch", pages: 880 }));
    // Beyond 2^53, JSON.parse would have rounded the integer.
    assert.equal(String(q.digestJson("9007199254740993")), digestText(9007199254740993n));
  });

  test("Digest text forms", () => {
    const d = q.digest({ title: "Middlemarch" });
    assert.ok(q.Digest.parse(String(d)).equals(d));
    assert.equal(d.bits, 256);
    assert.equal(d.micro().bits, 64);
    assert.equal(String(d.micro()), `sha256-64:${String(d).slice(7, 23)}`);
    assert.equal(JSON.stringify({ d }), `{"d":"${String(d)}"}`);
    assert.throws(() => d.micro().inDomain("x"), RangeError);
  });
});
