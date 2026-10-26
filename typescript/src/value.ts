/**
 * JavaScript values as values of the data model (spec 2), as
 * `history/TYPESCRIPT.md` (TS3) maps them, and as source values (spec 5.2)
 * for identities.
 */

import { Digest } from "./digest.js";
import { QuiddityError } from "./errors.js";
import { Duration, Instant, type Value, ValueMap, ValueSet, encode, normalize } from "./model.js";

/**
 * The key of a class's static identity: a class with
 * `static [quiddity.identity] = …` is digested under that identity, an
 * `Identity` or a kind built with quiddity's builders.
 */
export const identity: unique symbol = Symbol.for("quiddity.identity");

/** What a class's static identity provides: reading an instance, and its domain. */
export interface ReadsInstances {
  readInstance(x: object, path: string): Value;
  readonly instanceDomain: string | undefined;
}

/** The static identity of `x`'s class, if it has one. */
export function staticIdentity(x: object): ReadsInstances | undefined {
  const ctor = (x as { constructor?: unknown }).constructor;
  if (typeof ctor !== "function" || ctor === Object) return undefined;
  const found = (ctor as unknown as Record<symbol, unknown>)[identity];
  if (found === undefined) return undefined;
  const reader = (found as { [READER]?: () => ReadsInstances })[READER];
  if (typeof reader !== "function") {
    throw new QuiddityError(`the static [quiddity.identity] of ${ctor.name} is not an identity or a kind`);
  }
  return reader.call(found);
}

/** The method by which an `Identity` or a kind gives the reader of a static identity. */
export const READER: unique symbol = Symbol("quiddity.reader");

/** The name of `x`'s type, for messages. */
export function typeName(x: unknown): string {
  if (x === null) return "null";
  if (typeof x !== "object") return typeof x;
  const tag = (x as { [Symbol.toStringTag]?: unknown })[Symbol.toStringTag];
  if (typeof tag === "string") return tag;
  const ctor = (x as { constructor?: { name?: unknown } }).constructor;
  return typeof ctor?.name === "string" && ctor.name ? ctor.name : "object";
}

/** Whether `x` is a plain object: made by `{}`, `Object.create(null)` or `JSON.parse`. */
export function isPlainObject(x: object): boolean {
  const proto = Object.getPrototypeOf(x);
  return proto === null || proto === Object.prototype;
}

/** The kind of Temporal object `x` is, if it is one: `"Instant"`, `"Duration"`, …. */
export function temporalKind(x: object): string | undefined {
  const tag = (x as { [Symbol.toStringTag]?: unknown })[Symbol.toStringTag];
  return typeof tag === "string" && tag.startsWith("Temporal.") ? tag.slice(9) : undefined;
}

interface TemporalDuration {
  years: number; months: number; weeks: number; days: number;
  hours: number; minutes: number; seconds: number;
  milliseconds: number; microseconds: number; nanoseconds: number;
}

/**
 * A `Temporal.Duration` as integer nanoseconds: weeks of 7 days, days of
 * 86 400 seconds, months of 2 629 746 seconds and years of 12 months, the
 * mean Gregorian month and year.
 */
export function temporalDurationNs(d: TemporalDuration): bigint {
  const b = (n: number) => BigInt(n);
  return (
    ((b(d.years) * 12n + b(d.months)) * 2_629_746n +
      (b(d.weeks) * 7n + b(d.days)) * 86_400n +
      b(d.hours) * 3_600n +
      b(d.minutes) * 60n +
      b(d.seconds)) *
      1_000_000_000n +
    b(d.milliseconds) * 1_000_000n +
    b(d.microseconds) * 1_000n +
    b(d.nanoseconds)
  );
}

/** A `Temporal.PlainDateTime` as the instant it would be in UTC (spec 2.6). */
export function plainDateTimeNs(d: { toZonedDateTime(timeZone: string): { epochNanoseconds: bigint } }): bigint {
  return d.toZonedDateTime("UTC").epochNanoseconds;
}

interface TemporalPlainTime {
  hour: number; minute: number; second: number;
  millisecond: number; microsecond: number; nanosecond: number;
}

/** A `Temporal.PlainTime` as integer nanoseconds since midnight (spec 2.6). */
export function plainTimeNs(t: TemporalPlainTime): bigint {
  const b = (n: number) => BigInt(n);
  return (
    (b(t.hour) * 3_600n + b(t.minute) * 60n + b(t.second)) * 1_000_000_000n +
    b(t.millisecond) * 1_000_000n +
    b(t.microsecond) * 1_000n +
    b(t.nanosecond)
  );
}

/** The instant of a `Date`, or an error if it is invalid. */
export function dateNs(d: Date, path: string): bigint {
  const ms = d.getTime();
  if (!Number.isFinite(ms)) throw new QuiddityError("an invalid Date has no instant", path);
  return BigInt(ms) * 1_000_000n;
}

/** The path of the member or entry with key `key` inside `path`. */
export function keyPath(path: string, key: unknown): string {
  return typeof key === "string" ? `${path}.${key}` : `${path}[${typeof key === "bigint" ? key : String(key)}]`;
}

/**
 * The data-model value of `x`, or a {@link QuiddityError} if it has none.
 * A class with a static identity is read under it.
 */
export function fromJs(x: unknown, path = "$", active = new Set<object>()): Value {
  switch (typeof x) {
    case "boolean":
    case "bigint":
    case "string":
      return x;
    case "number":
      return normalize(x);
    case "undefined":
      throw new QuiddityError("undefined has no data-model value", path);
    case "object":
      if (x === null) return null;
      break;
    default:
      throw new QuiddityError(`a ${typeof x} has no data-model value`, path);
  }
  if (active.has(x)) throw new QuiddityError("the value contains itself", path);
  active.add(x);
  try {
    return objectFromJs(x, path, active);
  } finally {
    active.delete(x);
  }
}

function objectFromJs(x: object, path: string, active: Set<object>): Value {
  if (Array.isArray(x)) {
    const out: Value[] = [];
    for (let i = 0; i < x.length; i++) out.push(fromJs(x[i], `${path}[${i}]`, active));
    return out;
  }
  if (isPlainObject(x)) {
    const entries: [Value, Value][] = [];
    for (const [k, v] of Object.entries(x)) {
      // As JSON.stringify writes it: a member whose value is undefined is absent.
      if (v !== undefined) entries.push([k, fromJs(v, keyPath(path, k), active)]);
    }
    return new ValueMap(entries);
  }
  if (x instanceof Uint8Array) return x;
  if (x instanceof ArrayBuffer) return new Uint8Array(x);
  if (x instanceof DataView) return new Uint8Array(x.buffer, x.byteOffset, x.byteLength);
  if (x instanceof Set) {
    return new ValueSet([...x].map((item) => fromJs(item, `${path}[]`, active)));
  }
  if (x instanceof Map) {
    const entries: [Value, Value][] = [];
    for (const [k, v] of x) {
      const key = fromJs(k, path, active);
      entries.push([key, fromJs(v, keyPath(path, k), active)]);
    }
    checkKeys(entries, path);
    return new ValueMap(entries);
  }
  if (x instanceof Date) return new Instant(dateNs(x, path));
  if (x instanceof ValueSet || x instanceof ValueMap || x instanceof Instant || x instanceof Duration) return x;
  if (x instanceof Digest) return x.toString();
  if (typeName(x) === "URL") return (x as { href: string }).href;
  const temporal = temporalKind(x);
  if (temporal !== undefined) return temporalFromJs(x, temporal, path);
  const own = staticIdentity(x);
  if (own !== undefined) return own.readInstance(x, path);
  if (ArrayBuffer.isView(x)) {
    throw new QuiddityError(`a ${typeName(x)} has no data-model value: its bytes depend on the platform's byte order; use a Uint8Array`, path);
  }
  throw new QuiddityError(
    `a ${typeName(x)} has no data-model value; give its class a static [quiddity.identity]`,
    path,
  );
}

function temporalFromJs(x: object, kind: string, path: string): Value {
  switch (kind) {
    case "Instant":
    case "ZonedDateTime":
      return new Instant((x as { epochNanoseconds: bigint }).epochNanoseconds);
    case "PlainDateTime":
      return new Instant(plainDateTimeNs(x as Parameters<typeof plainDateTimeNs>[0]));
    case "Duration":
      return new Duration(temporalDurationNs(x as TemporalDuration));
    case "PlainTime":
      return new Duration(plainTimeNs(x as TemporalPlainTime));
    case "PlainDate":
    case "PlainYearMonth":
    case "PlainMonthDay":
      return String(x);
    default:
      throw new QuiddityError(`a Temporal.${kind} has no data-model value`, path);
  }
}

/** Throws if two keys of a map are the same data-model value, as `1` and `1n` are. */
export function checkKeys(entries: readonly (readonly [Value, Value])[], path: string): void {
  if (entries.length < 2) return;
  const seen = new Set<string>();
  for (const [k] of entries) {
    const e = encode(k).join(",");
    if (seen.has(e)) throw new QuiddityError(`the key ${String(k)} is given twice`, path);
    seen.add(e);
  }
}
