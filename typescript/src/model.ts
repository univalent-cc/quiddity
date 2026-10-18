/**
 * Values of the data model (spec 2) and their encoding (spec 3).
 *
 * JavaScript values are read as data-model values first (`value.ts`); the
 * encoding is computed from those. A data-model value is one of:
 *
 *     null, boolean          null, boolean
 *     bigint                 integer
 *     number                 float: never a finite integer once normalized
 *     string, Uint8Array     string: text (its WTF-8 bytes) or bytes
 *     Value[]                list
 *     ValueSet               set: order does not count, repeats do
 *     ValueMap               map: a list of [key, value] entries
 *     Instant, Duration      integer nanoseconds, kept apart from integers
 *                            so that identities can accept them as they are
 */

/** A value of the data model (spec 2.1). */
export type Value =
  | null
  | boolean
  | bigint
  | number
  | string
  | Uint8Array
  | Value[]
  | ValueSet
  | ValueMap
  | Instant
  | Duration;

/**
 * A set of the data model: a multiset, whose order does not count (spec
 * 2.1). A JavaScript `Set` is read as one; `ValueSet` can also hold members
 * that are equal, and members that are not JavaScript values.
 */
export class ValueSet {
  readonly items: readonly Value[];

  constructor(items: Iterable<Value>) {
    this.items = [...items];
  }
}

/**
 * A map of the data model: a multiset of entries, whose order does not count
 * (spec 2.1). A JavaScript `Map` or plain object is read as one; `ValueMap`
 * can also hold entries with equal keys.
 */
export class ValueMap {
  readonly entries: readonly (readonly [Value, Value])[];

  constructor(entries: Iterable<readonly [Value, Value]>) {
    this.entries = [...entries];
  }
}

/**
 * An instant: integer nanoseconds since 1970-01-01T00:00:00Z (spec 2.6). A
 * `Date` is read as one, to the millisecond; `Instant` holds nanoseconds.
 */
export class Instant {
  readonly ns: bigint;

  constructor(ns: bigint) {
    this.ns = ns;
  }

  /** The instant of a `Date`, which has millisecond precision. */
  static fromDate(date: Date): Instant {
    const ms = date.getTime();
    if (!Number.isFinite(ms)) throw new RangeError("an invalid Date has no instant");
    return new Instant(BigInt(ms) * 1_000_000n);
  }
}

/** A duration: integer nanoseconds (spec 2.6). */
export class Duration {
  readonly ns: bigint;

  constructor(ns: bigint) {
    this.ns = ns;
  }
}

const T_NULL = 0x01, T_FALSE = 0x02, T_TRUE = 0x03;
const T_INT = 0x10, T_FLOAT = 0x11;
const T_STR = 0x20;
const T_LIST = 0x30, T_SET = 0x31, T_MAP = 0x32;

/**
 * A number as the data model holds it (spec 2.2): a finite number with an
 * integer value is that integer, as a bigint; any other number is a float.
 */
export function normalize(x: number): bigint | number {
  return Number.isInteger(x) ? BigInt(x) : x;
}

/**
 * Text as bytes (spec 2.3): UTF-8, with surrogate pairs combined and each
 * lone surrogate written as UTF-8 would write its code point (WTF-8).
 */
export function wtf8(s: string): Uint8Array {
  const out = new Uint8Array(s.length * 3);
  let n = 0;
  for (let i = 0; i < s.length; i++) {
    let c = s.charCodeAt(i);
    if (c < 0x80) {
      out[n++] = c;
      continue;
    }
    if (c >= 0xd800 && c < 0xdc00 && i + 1 < s.length) {
      const d = s.charCodeAt(i + 1);
      if (d >= 0xdc00 && d < 0xe000) {
        c = 0x10000 + ((c - 0xd800) << 10) + (d - 0xdc00);
        i++;
      }
    }
    if (c < 0x800) {
      out[n++] = 0xc0 | (c >> 6);
      out[n++] = 0x80 | (c & 0x3f);
    } else if (c < 0x10000) {
      out[n++] = 0xe0 | (c >> 12);
      out[n++] = 0x80 | ((c >> 6) & 0x3f);
      out[n++] = 0x80 | (c & 0x3f);
    } else {
      out[n++] = 0xf0 | (c >> 18);
      out[n++] = 0x80 | ((c >> 12) & 0x3f);
      out[n++] = 0x80 | ((c >> 6) & 0x3f);
      out[n++] = 0x80 | (c & 0x3f);
    }
  }
  return out.slice(0, n);
}

/** Minimal two's-complement big-endian bytes of `n` (spec 3.4). */
export function intBytes(n: bigint): Uint8Array {
  // The number of bytes: the least L with -2^(8L-1) <= n < 2^(8L-1).
  const magnitude = n < 0n ? -n - 1n : n;
  const bits = magnitude === 0n ? 0 : magnitude.toString(2).length;
  const length = Math.floor(bits / 8) + 1;
  let u = n < 0n ? (1n << BigInt(length * 8)) + n : n;
  const out = new Uint8Array(length);
  for (let i = length - 1; i >= 0; i--) {
    out[i] = Number(u & 0xffn);
    u >>= 8n;
  }
  return out;
}

/** Bytes written in sequence, as an encoding is (spec 3). */
class Writer {
  private buf = new Uint8Array(256);
  private len = 0;

  private room(n: number): void {
    if (this.len + n <= this.buf.length) return;
    let size = this.buf.length * 2;
    while (size < this.len + n) size *= 2;
    const next = new Uint8Array(size);
    next.set(this.buf.subarray(0, this.len));
    this.buf = next;
  }

  byte(b: number): void {
    this.room(1);
    this.buf[this.len++] = b;
  }

  bytes(b: Uint8Array): void {
    this.room(b.length);
    this.buf.set(b, this.len);
    this.len += b.length;
  }

  u64(n: number): void {
    this.room(8);
    const hi = Math.floor(n / 0x100000000);
    const lo = n >>> 0;
    const b = this.buf;
    let i = this.len;
    b[i++] = hi >>> 24; b[i++] = (hi >>> 16) & 0xff; b[i++] = (hi >>> 8) & 0xff; b[i++] = hi & 0xff;
    b[i++] = lo >>> 24; b[i++] = (lo >>> 16) & 0xff; b[i++] = (lo >>> 8) & 0xff; b[i++] = lo & 0xff;
    this.len = i;
  }

  done(): Uint8Array {
    return this.buf.slice(0, this.len);
  }
}

/** Byte strings compared lexicographically (spec 2.5). */
export function compareBytes(a: Uint8Array, b: Uint8Array): number {
  const n = Math.min(a.length, b.length);
  for (let i = 0; i < n; i++) {
    if (a[i] !== b[i]) return a[i]! - b[i]!;
  }
  return a.length - b.length;
}

const floatView = new DataView(new ArrayBuffer(8));

/** The encoding E(v) of a data-model value (spec 3). */
export function encode(v: Value): Uint8Array {
  const w = new Writer();
  write(v, w);
  return w.done();
}

function write(v: Value, w: Writer): void {
  if (v === null) {
    w.byte(T_NULL);
  } else if (typeof v === "boolean") {
    w.byte(v ? T_TRUE : T_FALSE);
  } else if (typeof v === "bigint") {
    writeInt(v, w);
  } else if (typeof v === "number") {
    if (Number.isInteger(v)) {
      writeInt(BigInt(v), w);
      return;
    }
    if (Number.isNaN(v)) {
      w.bytes(CANONICAL_NAN);
    } else {
      floatView.setFloat64(0, v);
      w.bytes(new Uint8Array(floatView.buffer.slice(0)));
    }
    w.byte(T_FLOAT);
  } else if (typeof v === "string") {
    const b = wtf8(v);
    w.bytes(b);
    w.u64(b.length);
    w.byte(T_STR);
  } else if (v instanceof Uint8Array) {
    w.bytes(v);
    w.u64(v.length);
    w.byte(T_STR);
  } else if (Array.isArray(v)) {
    for (const item of v) write(item, w);
    w.u64(v.length);
    w.byte(T_LIST);
  } else if (v instanceof ValueSet) {
    // Canonical order (spec 2.5): by encoding.
    const members = v.items.map(encode).sort(compareBytes);
    for (const m of members) w.bytes(m);
    w.u64(members.length);
    w.byte(T_SET);
  } else if (v instanceof ValueMap) {
    // Canonical order (spec 2.5): by the key's encoding, then the value's.
    const entries = v.entries.map(([k, x]) => [encode(k), encode(x)] as const);
    entries.sort((a, b) => compareBytes(a[0], b[0]) || compareBytes(a[1], b[1]));
    for (const [k, x] of entries) {
      w.bytes(k);
      w.bytes(x);
    }
    w.u64(entries.length);
    w.byte(T_MAP);
  } else if (v instanceof Instant || v instanceof Duration) {
    writeInt(v.ns, w);
  } else {
    throw new TypeError(`not a data-model value: ${String(v)}`);
  }
}

const CANONICAL_NAN = new Uint8Array([0x7f, 0xf8, 0, 0, 0, 0, 0, 0]);

function writeInt(n: bigint, w: Writer): void {
  const b = intBytes(n);
  w.bytes(b);
  w.u64(b.length);
  w.byte(T_INT);
}

/** Whether `v` is an empty string, list, set or map (spec 5.6). */
export function isEmpty(v: Value): boolean {
  if (typeof v === "string" || v instanceof Uint8Array || Array.isArray(v)) return v.length === 0;
  if (v instanceof ValueSet) return v.items.length === 0;
  if (v instanceof ValueMap) return v.entries.length === 0;
  return false;
}
