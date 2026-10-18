/**
 * JSON text (RFC 8259) read with the number rule of spec 2.7, and written
 * with bigints.
 *
 * `JSON.parse` reads every number as a binary64 value, so integers beyond
 * 2^53 lose their value, and it keeps the last of repeated member names.
 * Spec 2.7 keeps integers exact up to 64 bits and is defined for I-JSON,
 * which has no repeated names.
 */

import { QuiddityError } from "./errors.js";

/** A JSON number as its text, for readers that need more than its value. */
export class RawNumber {
  readonly text: string;

  constructor(text: string) {
    this.text = text;
  }

  /** Whether the text has no fraction and no exponent. */
  get isIntegerText(): boolean {
    return !/[.eE]/.test(this.text);
  }
}

/** A JSON value with each number as its text. */
export type RawJson = null | boolean | string | RawNumber | RawJson[] | { [member: string]: RawJson };

/** A JSON value as `parseJson` gives it. */
export type Json = null | boolean | number | bigint | string | Json[] | { [member: string]: Json };

const MIN_I64 = -(2n ** 63n);
const MAX_U64 = 2n ** 64n - 1n;

/** The value spec 2.7 gives a JSON number's text: an exact integer, or binary64. */
export function jsonNumber(text: string): number | bigint {
  if (/^-?(0|[1-9][0-9]*)$/.test(text)) {
    const n = BigInt(text);
    if (n >= MIN_I64 && n <= MAX_U64) {
      return n >= BigInt(Number.MIN_SAFE_INTEGER) && n <= BigInt(Number.MAX_SAFE_INTEGER) ? Number(n) : n;
    }
  }
  return Number(text);
}

/**
 * Reads JSON text as spec 2.7 maps it onto the data model: an integer whose
 * text has no fraction or exponent, from -2^63 to 2^64 - 1, is exact (a
 * `bigint` when it is beyond 2^53, a `number` otherwise); any other number
 * is the nearest binary64 value. A repeated member name is a
 * {@link QuiddityError}; text that is not JSON a `SyntaxError`.
 *
 * ```ts
 * import * as q from "quiddity";
 *
 * q.parseJson('{"isbn": 9780141439549, "copies": 18446744073709551615}');
 * // { isbn: 9780141439549, copies: 18446744073709551615n }
 * ```
 */
export function parseJson(text: string): Json {
  return parse(text, jsonNumber) as Json;
}

/** Reads JSON text, keeping each number's text. */
export function parseRawJson(text: string): RawJson {
  return parse(text, (t) => new RawNumber(t)) as RawJson;
}

type Parsed = null | boolean | string | number | bigint | RawNumber | Parsed[] | { [member: string]: Parsed };

function parse(text: string, number: (text: string) => Parsed): Parsed {
  let i = 0;

  const fail = (what: string): never => {
    throw new SyntaxError(`not JSON: ${what} at position ${i}`);
  };

  const space = (): void => {
    for (;;) {
      const c = text.charCodeAt(i);
      if (c === 0x20 || c === 0x09 || c === 0x0a || c === 0x0d) i++;
      else return;
    }
  };

  const literal = (word: string, v: Parsed): Parsed => {
    if (text.startsWith(word, i)) {
      i += word.length;
      return v;
    }
    return fail("an unexpected character");
  };

  const string = (): string => {
    i++; // the opening quote
    let out = "";
    let start = i;
    for (;;) {
      if (i >= text.length) fail("an unterminated string");
      const c = text.charCodeAt(i);
      if (c === 0x22) {
        out += text.slice(start, i);
        i++;
        return out;
      }
      if (c < 0x20) fail("a control character in a string");
      if (c !== 0x5c) {
        i++;
        continue;
      }
      out += text.slice(start, i);
      const e = text[i + 1];
      i += 2;
      switch (e) {
        case '"': out += '"'; break;
        case "\\": out += "\\"; break;
        case "/": out += "/"; break;
        case "b": out += "\b"; break;
        case "f": out += "\f"; break;
        case "n": out += "\n"; break;
        case "r": out += "\r"; break;
        case "t": out += "\t"; break;
        case "u": {
          const hex = text.slice(i, i + 4);
          if (!/^[0-9a-fA-F]{4}$/.test(hex)) fail("a bad \\u escape");
          out += String.fromCharCode(parseInt(hex, 16));
          i += 4;
          break;
        }
        default:
          fail("a bad escape");
      }
      start = i;
    }
  };

  const NUMBER = /-?(?:0|[1-9][0-9]*)(?:\.[0-9]+)?(?:[eE][+-]?[0-9]+)?/y;

  const value = (path: string): Parsed => {
    space();
    const c = text[i];
    if (c === "{") {
      i++;
      const out: { [member: string]: Parsed } = {};
      const seen = new Set<string>();
      space();
      if (text[i] === "}") {
        i++;
        return out;
      }
      for (;;) {
        space();
        if (text[i] !== '"') fail("expected a member name");
        const name = string();
        if (seen.has(name)) {
          throw new QuiddityError(`the member name ${JSON.stringify(name)} is repeated`, path);
        }
        seen.add(name);
        space();
        if (text[i] !== ":") fail("expected ':'");
        i++;
        const v = value(`${path}.${name}`);
        // A member named __proto__ is an ordinary member, as JSON.parse makes it.
        Object.defineProperty(out, name, { value: v, enumerable: true, writable: true, configurable: true });
        space();
        if (text[i] === ",") {
          i++;
          continue;
        }
        if (text[i] === "}") {
          i++;
          return out;
        }
        fail("expected ',' or '}'");
      }
    }
    if (c === "[") {
      i++;
      const out: Parsed[] = [];
      space();
      if (text[i] === "]") {
        i++;
        return out;
      }
      for (;;) {
        out.push(value(`${path}[${out.length}]`));
        space();
        if (text[i] === ",") {
          i++;
          continue;
        }
        if (text[i] === "]") {
          i++;
          return out;
        }
        fail("expected ',' or ']'");
      }
    }
    if (c === '"') return string();
    if (c === "t") return literal("true", true);
    if (c === "f") return literal("false", false);
    if (c === "n") return literal("null", null);
    NUMBER.lastIndex = i;
    const m = NUMBER.exec(text);
    if (!m) return fail(i >= text.length ? "unexpected end of text" : "an unexpected character");
    i += m[0].length;
    return number(m[0]);
  };

  const result = value("$");
  space();
  if (i < text.length) fail("text after the value");
  return result;
}

/**
 * JSON text for a value of plain objects, arrays, strings, booleans, null,
 * finite numbers and bigints, with members in sorted order if `sorted`.
 */
export function jsonText(v: unknown, sorted = false): string {
  if (v === null) return "null";
  if (typeof v === "boolean") return v ? "true" : "false";
  if (typeof v === "bigint") return v.toString();
  if (typeof v === "number") {
    if (!Number.isFinite(v)) throw new TypeError(`${v} is not JSON`);
    return Number.isInteger(v) && Math.abs(v) < 1e21 ? BigInt(v).toString() : String(v);
  }
  if (typeof v === "string") return JSON.stringify(v);
  if (v instanceof RawNumber) return v.text;
  if (Array.isArray(v)) return `[${v.map((x) => jsonText(x, sorted)).join(",")}]`;
  if (typeof v === "object") {
    let keys = Object.keys(v);
    if (sorted) keys = keys.sort(compareCodePoints);
    const record = v as Record<string, unknown>;
    return `{${keys.map((k) => `${JSON.stringify(k)}:${jsonText(record[k], sorted)}`).join(",")}}`;
  }
  throw new TypeError(`a ${typeof v} is not JSON`);
}

/** Strings compared by code point, as Python's `sorted` compares them. */
export function compareCodePoints(a: string, b: string): number {
  const x = [...a];
  const y = [...b];
  const n = Math.min(x.length, y.length);
  for (let i = 0; i < n; i++) {
    const d = x[i]!.codePointAt(0)! - y[i]!.codePointAt(0)!;
    if (d !== 0) return d;
  }
  return x.length - y.length;
}

/** A deep copy of a value of plain objects, arrays and scalars. */
export function copyJson<T>(v: T): T {
  if (Array.isArray(v)) return v.map(copyJson) as T;
  if (typeof v === "object" && v !== null && !(v instanceof RawNumber)) {
    const out: Record<string, unknown> = {};
    for (const [k, x] of Object.entries(v)) {
      Object.defineProperty(out, k, { value: copyJson(x), enumerable: true, writable: true, configurable: true });
    }
    return out as T;
  }
  return v;
}
