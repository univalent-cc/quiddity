/** Digests (spec 4). */

import { wtf8 } from "./model.js";
import { sha256 } from "./sha256.js";

const VALUE_PREFIX = ascii("quiddity/1/value");
const DOMAIN_PREFIX = ascii("quiddity/1/domain");
const PREFIXES: Record<number, string> = { 32: "sha256:", 16: "sha256-128:", 8: "sha256-64:" };
const SIZES: Record<string, number> = { "sha256": 32, "sha256-128": 16, "sha256-64": 8 };
const TEXT = /^(sha256|sha256-128|sha256-64):([0-9a-fA-F]+)$/;

function ascii(s: string): Uint8Array {
  return Uint8Array.from(s, (c) => c.charCodeAt(0));
}

function u64be(n: number): Uint8Array {
  const out = new Uint8Array(8);
  const view = new DataView(out.buffer);
  view.setUint32(0, Math.floor(n / 0x100000000));
  view.setUint32(4, n >>> 0);
  return out;
}

function hex(bytes: Uint8Array): string {
  let s = "";
  for (const b of bytes) s += b.toString(16).padStart(2, "0");
  return s;
}

/**
 * A digest: the 256-bit SHA-256 digest of a value's encoding (spec 4), or
 * one of its shortened forms of 128 or 64 bits (spec 4.3).
 *
 * A digest is immutable. `String(digest)` is its text form (spec 4.4),
 * which {@link Digest.parse} reads back, and `JSON.stringify` writes that
 * text too.
 *
 * ```ts
 * import * as q from "quiddity";
 *
 * const d = q.digest({ title: "Middlemarch", pages: 880 });
 * String(d.micro()); // "sha256-64:…", short enough for an ETag
 * q.Digest.parse(String(d)).equals(d); // true
 * ```
 */
export class Digest {
  readonly #bytes: Uint8Array;

  /** A digest of 32, 16 or 8 bytes. */
  constructor(bytes: Uint8Array) {
    if (!(bytes.length in PREFIXES)) {
      throw new RangeError(`a digest is 32, 16 or 8 bytes, not ${bytes.length}`);
    }
    this.#bytes = Uint8Array.from(bytes);
    Object.freeze(this);
  }

  /** The structural digest S(v) of a value whose encoding is `encoding` (spec 4.1). */
  static ofEncoding(encoding: Uint8Array): Digest {
    return new Digest(sha256(VALUE_PREFIX, encoding));
  }

  /**
   * Reads a digest's text form (spec 4.4), throwing a `SyntaxError` if
   * `text` is not one. Hex digits may be upper case.
   */
  static parse(text: string): Digest {
    const m = TEXT.exec(text);
    const size = m ? SIZES[m[1]!]! : 0;
    if (!m || m[2]!.length !== size * 2) throw new SyntaxError(`not a digest: ${JSON.stringify(text)}`);
    const bytes = new Uint8Array(size);
    for (let i = 0; i < size; i++) bytes[i] = parseInt(m[2]!.slice(i * 2, i * 2 + 2), 16);
    return new Digest(bytes);
  }

  /** The digest's bytes: 32, 16 or 8 of them. */
  get bytes(): Uint8Array {
    return Uint8Array.from(this.#bytes);
  }

  /** The digest's size in bits: 256, 128 or 64. */
  get bits(): number {
    return this.#bytes.length * 8;
  }

  /** The 128-bit form: the first 16 bytes (spec 4.3). */
  mini(): Digest {
    return this.#shorten(16);
  }

  /** The 64-bit form: the first 8 bytes (spec 4.3), short enough for an ETag. */
  micro(): Digest {
    return this.#shorten(8);
  }

  #shorten(size: number): Digest {
    if (this.#bytes.length < size) {
      throw new RangeError(`a ${this.bits}-bit digest cannot be shortened to ${size * 8} bits`);
    }
    return new Digest(this.#bytes.subarray(0, size));
  }

  /**
   * This digest placed in the domain `label` (spec 4.2). Digests in different
   * domains never collide, so values of different types with the same
   * fields have different digests in their own domains.
   */
  inDomain(label: string | Uint8Array): Digest {
    if (this.#bytes.length !== 32) throw new RangeError("only a 256-bit digest can be placed in a domain");
    const lb = typeof label === "string" ? wtf8(label) : label;
    if (lb.length === 0) throw new RangeError("a domain label is not empty");
    return new Digest(sha256(DOMAIN_PREFIX, lb, u64be(lb.length), this.#bytes));
  }

  /** Whether `other` is a digest with the same bytes. */
  equals(other: unknown): boolean {
    if (!(other instanceof Digest)) return false;
    const a = this.#bytes;
    const b = other.#bytes;
    return a.length === b.length && a.every((x, i) => x === b[i]);
  }

  /** The text form (spec 4.4): `sha256:` and 64 lowercase hex digits, or a shortened form. */
  toString(): string {
    return PREFIXES[this.#bytes.length] + hex(this.#bytes);
  }

  /** The text form, for `JSON.stringify`. */
  toJSON(): string {
    return this.toString();
  }
}
