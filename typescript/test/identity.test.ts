// Identities: built in code, read from JSON, changed, and applied.

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { describe, test } from "node:test";

import * as q from "../dist/index.js";
import { IDENTITY_OF_IDENTITIES } from "../dist/meta.js";

const SPEC = readFileSync(new URL("../../QUIDDITY_SPEC.md", import.meta.url), "utf8");

/** The JSON example in the spec that contains `marker`. */
function specJson(marker: string): string {
  const blocks = [...SPEC.matchAll(/```json\n([\s\S]*?)```/g)].map((m) => m[1]!);
  const found = blocks.filter((b) => b.includes(marker));
  assert.equal(found.length, 1);
  return found[0]!;
}

describe("identities", () => {
  test("the identity of identities is the spec's Appendix C", () => {
    assert.equal(IDENTITY_OF_IDENTITIES, specJson('"quiddity.identity"').trimEnd());
  });

  test("the spec's example, built in code, is the spec's JSON", () => {
    const Row = q.object({ label: q.string(), value: q.number() }).named("Row");
    const Question = q
      .object({
        brief: q.string(),
        rows: q.list(Row),
        tags: q.set(q.string()).omit("empty"),
        note: q.optional(q.string()).omit("none"),
        asked_at: q.instant(),
        kind: q.string().rename("type"),
      })
      .named("Question");
    const built = Question.inDomain("example.question");
    const read = q.Identity.fromJson(specJson('"example.question"'));
    assert.ok(built.equals(read));
    assert.ok(q.digest(built).equals(q.digest(read)));
    assert.equal(built.name, "Question");
    assert.equal(built.domain, "example.question");
  });

  test("fromJson reads text strictly, and objects from JSON.parse", () => {
    const text = '{"root": {"object": {"fields": {"n": {"kind": "integer", "quantize": 10}}}}}';
    assert.ok(q.Identity.fromJson(text).equals(q.Identity.fromJson(JSON.parse(text))));
    assert.throws(() => q.Identity.fromJson(text.replace("10", "10.0")), /expected an integer, not 10.0/);
    assert.throws(() => q.Identity.fromJson('{"root": "string", "root": "integer"}'), q.InvalidIdentity);
    assert.throws(() => q.Identity.fromJson("{"), q.InvalidIdentity);
  });

  test("an identity's digest is the same however it is written", () => {
    const a = q.Identity.fromJson({ root: { union: ["string", "null"] }, definitions: { Unused: "integer" } });
    const b = new q.Identity(q.optional(q.string()));
    assert.ok(a.equals(b));
    assert.deepEqual(a.toJson(), { root: { union: ["null", "string"] } });
    assert.equal(String(q.digest(a)), String(q.digest(b)));
  });

  test("applying an identity reads, converts and leaves out", () => {
    const Book = q.object({
      title: q.string(),
      pages: q.integer(),
      published: q.instant(),
      genres: q.optional(q.set(q.string())).omit("none", "empty"),
    });
    // Members the identity does not list are left out. (A fresh object
    // literal with one would be a type error, so this one is a variable.)
    const onShelf = { title: "Moby Dick", pages: 635, published: new Date("1851-10-18T00:00:00Z"), shelf: "B4" };
    const a = Book.digest(onShelf);
    const b = Book.digest({ title: "Moby Dick", pages: "635", published: "1851-10-18T01:00:00+01:00", genres: [] });
    assert.ok(a.equals(b));
    // An object for an instant must be serde's form of Rust's SystemTime.
    assert.throws(
      // @ts-expect-error: not an instant
      () => Book.digest({ title: "Moby Dick", pages: 635, published: { year: 1851 } }),
      /expected exactly the members secs_since_epoch and nanos_since_epoch \(at \$\.published\)/,
    );
  });

  test("errors say where", () => {
    const Shelf = q.object({ books: q.list(q.object({ title: q.string(), pages: q.integer() })) });
    assert.throws(
      () => Shelf.digest({ books: [{ title: "Moby Dick", pages: 635 }, { title: "Typee", pages: "many" }] }),
      (e: unknown) => e instanceof q.QuiddityError && e.path === "$.books[1].pages" && /not an integer/.test(e.detail),
    );
    assert.throws(
      // @ts-expect-error: a book without a title
      () => Shelf.digest({ books: [{ pages: 1 }] }),
      (e: unknown) => e instanceof q.QuiddityError && e.path === "$.books[0].title",
    );
  });

  test("a class's static identity, in its domain at the top level only", () => {
    class Book {
      static [q.identity] = q
        .object({ title: q.string(), pages: q.integer() })
        .inDomain("example.book");
      title: string;
      pages: number;
      lastOpened = new Date();
      constructor(title: string, pages: number) {
        this.title = title;
        this.pages = pages;
      }
    }
    const mobyDick = new Book("Moby Dick", 635);
    const plain = q.digest({ title: "Moby Dick", pages: 635 });
    assert.ok(q.digest(mobyDick).equals(plain.inDomain("example.book")));
    // Nested, a value has no domain (spec 4.2).
    assert.ok(q.digest([mobyDick]).equals(q.digest([{ title: "Moby Dick", pages: 635 }])));
    // Under another identity, an instance is read as its own members.
    const Title = q.object({ title: q.string() });
    assert.ok(Title.digest(mobyDick).equals(Title.digest({ title: "Moby Dick" })));
  });

  test("an identity is digested as its JSON form under the identity of identities", () => {
    const identity = q.Identity.fromJson('{"root": {"list": "string"}}');
    const meta = q.Identity.fromJson(IDENTITY_OF_IDENTITIES);
    assert.ok(q.digest(identity).equals(meta.digest(identity.toJson())));
  });

  test("named, withDomain, withField and withoutField", () => {
    const base = q.object({ title: q.string() }).identity;
    const named = base.named("Book");
    assert.deepEqual(named.toJson(), {
      root: { ref: "Book" },
      definitions: { Book: { object: { fields: { title: { kind: "string" } } } } },
    });
    assert.ok(named.named("Volume").toJson().definitions!["Volume"]);
    const withPages = named.withField("pages", q.integer().quantize(10));
    assert.deepEqual(withPages.toJson().definitions!["Book"], {
      object: { fields: { title: { kind: "string" }, pages: { kind: "integer", quantize: 10 } } },
    });
    assert.ok(withPages.withoutField("pages").equals(named));
    assert.equal(base.withDomain("example.book").domain, "example.book");
    assert.throws(() => base.withoutField("author"), /there is no field named "author"/);
    assert.throws(() => base.withField("pages", q.string().quantize(10)), /quantize does not apply/);
  });

  test("recursive kinds refer to themselves by name", () => {
    const Section = q.object({ heading: q.string(), sections: q.list(q.ref("Section")) }).named("Section");
    const doc = { heading: "Middlemarch", sections: [{ heading: "Book I", sections: [] }] };
    assert.ok(Section.digest(doc));
    assert.throws(() => new q.Identity(q.list(q.ref("Chapter"))), /ref to undefined "Chapter"/);
  });

  test("two different kinds with one name are an error", () => {
    const a = q.object({ title: q.string() }).named("Book");
    const b = q.object({ isbn: q.string() }).named("Book");
    assert.throws(() => new q.Identity(q.tuple(a, b)), /two different kinds are named "Book"/);
    assert.ok(new q.Identity(q.tuple(a, a)));
  });

  test("tagged unions, built in code", () => {
    const Format = q.tagged(
      {
        hardcover: q.object({ pages: q.integer() }),
        audiobook: q.object({ minutes: q.integer() }),
        ebook: null,
      },
      { style: "internal", tag: "format" },
    );
    const a = Format.digest({ format: "hardcover", pages: 635, price: 12 });
    const b = Format.digest({ pages: "635", format: "hardcover" });
    assert.ok(a.equals(b));
    assert.ok(Format.digest({ format: "ebook" }));
    assert.throws(() => Format.digest({ format: "scroll" }), /unknown variant "scroll"/);

    const Rating = q.tagged(
      { liked: q.variant(null, { discriminant: 1 }), disliked: q.variant(null, { discriminant: 2n ** 63n }) },
      { encodeAs: "int" },
    );
    assert.match(Rating.identity.toJsonText(), /"discriminant":9223372036854775808/);
    assert.ok(q.Identity.fromJson(Rating.identity.toJsonText()).equals(Rating.identity));
  });

  test("objects told apart by their keys", () => {
    const Size = q.union(q.object({ width: q.integer(), height: q.integer() }), q.object({ pages: q.integer() }));
    assert.notEqual(String(Size.digest({ pages: 3 })), String(Size.digest({ width: 3, height: 4 })));
    assert.throws(() => Size.digest({ pages: 3, width: 3 }), /fits none/);
  });

  test("quantization", () => {
    const Price = q.object({ amount: q.number().decimals(2), at: q.instant().quantize(1_000_000_000n) });
    const a = Price.digest({ amount: 12.989, at: "2024-01-01T00:00:00.9Z" });
    const b = Price.digest({ amount: 12.98, at: "2024-01-01T00:00:00Z" });
    assert.ok(a.equals(b));
  });
});
