// What the compiler checks: `npm run typecheck` fails if a line marked with
// a ts-expect-error comment is not an error. The tests also run, to check
// that the kinds these types describe behave as the types say.

import assert from "node:assert/strict";
import { describe, test } from "node:test";

import * as q from "../dist/index.js";

/** Compiles only if `A` and `B` are the same type. */
type Equal<A, B> = (<T>() => T extends A ? 1 : 2) extends <T>() => T extends B ? 1 : 2 ? true : false;
function expectType<T extends true>(): T | undefined {
  return undefined;
}

interface Book {
  title: string;
  pages: number;
  genres?: string[];
  lastOpened: Date;
}

const bookFields = {
  title: q.string(),
  pages: q.integer(),
  genres: q.optional(q.set(q.string())).omit("none", "empty"),
  lastOpened: q.skip(),
};

describe("objectFor", () => {
  test("names every member of the type", () => {
    const BookIdentity = q.objectFor<Book>(bookFields);
    assert.deepEqual(Object.keys(BookIdentity.toJson().root as object), ["object"]);

    // A member of Book that the identity does not name.
    // @ts-expect-error: property 'lastOpened' is missing
    q.objectFor<Book>({ title: q.string(), pages: q.integer(), genres: q.optional(q.list(q.string())) });

    // A field that Book does not have.
    // @ts-expect-error: 'titel' does not exist in type FieldsFor<Book>
    q.objectFor<Book>({ ...bookFields, titel: q.string() });

    // A kind that does not read the member's type.
    // @ts-expect-error: a string kind does not read a number
    q.objectFor<Book>({ ...bookFields, pages: q.string() });

    // An optional member needs a kind that reads a missing member.
    // @ts-expect-error: undefined is not assignable to a set's values
    q.objectFor<Book>({ ...bookFields, genres: q.set(q.string()) });
  });

  test("ignores methods and checks nested types", () => {
    class Shelf {
      label = "B4";
      books: Book[] = [];
      count(): number {
        return this.books.length;
      }
    }
    const BookKind = q.objectFor<Book>(bookFields);
    const ShelfKind = q.objectFor<Shelf>({ label: q.string(), books: q.list(BookKind) });
    assert.ok(ShelfKind.digest(new Shelf()));

    // @ts-expect-error: a list of strings does not read a list of books
    q.objectFor<Shelf>({ label: q.string(), books: q.list(q.string()) });
  });

  test("kinds that convert read more than they describe", () => {
    interface Imported {
      pages: string; // a CSV import: numbers as text
    }
    // An integer kind reads "880" (spec 5.5), so this is allowed.
    const Pages = q.objectFor<Imported>({ pages: q.integer() });
    assert.ok(Pages.digest({ pages: "880" }).equals(q.object({ pages: q.integer() }).digest({ pages: 880 })));
  });
});

describe("inference", () => {
  test("object kinds describe objects", () => {
    const Book = q.object({
      title: q.string(),
      pages: q.integer(),
      genres: q.optional(q.set(q.string())),
      published: q.instant(),
    });
    type Book = q.Infer<typeof Book>;
    expectType<Equal<Book, { title: string; pages: number | bigint; published: Date; genres?: Set<string> | null }>>();
    const b: q.Accepts<typeof Book> = { title: "Moby Dick", pages: "635", published: "1851-10-18T00:00:00Z" };
    assert.ok(Book.digest(b));
  });

  test("collections and unions", () => {
    const Row = q.tuple(q.string(), q.integer());
    expectType<Equal<q.Infer<typeof Row>, [string, number | bigint]>>();
    const Either = q.union(q.integer(), q.string());
    expectType<Equal<q.Infer<typeof Either>, string | number | bigint>>();
    const Index = q.map(q.string(), q.list(q.integer()));
    expectType<Equal<q.Infer<typeof Index>, Record<string, (number | bigint)[]>>>();
    assert.ok(Index.digest({ a: [1, 2] }));
  });

  test("digest checks its argument's type", () => {
    const Title = q.object({ title: q.string() });
    // @ts-expect-error: a number is not a title
    assert.throws(() => Title.digest({ title: 5 }), q.QuiddityError);
  });
});
