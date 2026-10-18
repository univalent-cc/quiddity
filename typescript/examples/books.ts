// Identities built in code, checked against a type: the book example from
// the README.

import * as q from "quiddity";

interface Book {
  title: string;
  pages: number;
  genres?: string[];
  lastOpened: Date; // when you last read it: not part of the book
}

const BookIdentity = q.objectFor<Book>({
  title: q.string(),
  pages: q.integer(),
  genres: q.optional(q.set(q.string())).omit("none", "empty"),
  lastOpened: q.skip(),
}).inDomain("example.book");

const mine: Book = { title: "Middlemarch", pages: 880, genres: ["realist", "novel"], lastOpened: new Date() };
const yours: Book = { title: "Middlemarch", pages: 880, genres: ["novel", "realist"], lastOpened: new Date(0) };

// The same book, whenever each copy was last opened, and in any genre order.
console.log(BookIdentity.digest(mine).equals(BookIdentity.digest(yours))); // true

// The identity as data, for any other language to apply to the same JSON.
console.log(BookIdentity.toJsonText());
console.log(String(q.digest(BookIdentity))); // the identity's own digest
