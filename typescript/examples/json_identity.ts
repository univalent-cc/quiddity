// An identity read from its JSON form, applied to JSON documents: what
// another language, given the same identity file, digests the same way.

import { readFileSync } from "node:fs";

import * as q from "quiddity";

const read = (name: string) => readFileSync(new URL(`data/${name}`, import.meta.url), "utf8");

const book = q.Identity.fromJson(read("book.identity.json"));

// Two documents that differ only in what the identity leaves out or
// converts: member order, the order of a set, "880" for 880, lastOpened.
const a = q.digestJson(read("book.json"), book);
const b = q.digestJson(read("book-reordered.json"), book);
console.log(a.equals(b)); // true
console.log(String(a));

// The same identity, built in code, has the same digest as the file's.
const built = q
  .object({
    title: q.string(),
    pages: q.integer(),
    genres: q.optional(q.set(q.string())).omit("none", "empty"),
  })
  .inDomain("example.book");
console.log(q.digest(built).equals(q.digest(book))); // true
