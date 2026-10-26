/**
 * Applying an identity to a source value (spec 5.4 to 5.8). Follows
 * `test-vectors/qident.py`.
 *
 * Sources are JavaScript values, read as the source types of spec 5.2 (see
 * `sourceType`). An object source is a plain object or a class instance
 * (its own enumerable properties, as `JSON.stringify` reads them), a `Map`
 * or a `ValueMap`.
 */

import { Digest } from "./digest.js";
import { QuiddityError } from "./errors.js";
import { jsonNumber } from "./json.js";
import { type Checker, type Kind, type SourceType, UNITS } from "./load.js";
import { Duration, Instant, type Value, ValueMap, ValueSet, isEmpty, normalize, wtf8 } from "./model.js";
import {
  BOOLS, NUMBER_WORDS, type Rational, base64Bytes, decimal, decimalRational, floatRational, floorDiv, hexBytes,
  isoDuration, quantizeDecimals, rfc3339, trimSpace, uuid,
} from "./text.js";
import {
  dateNs,
  fromJs,
  isPlainObject,
  keyPath,
  plainDateTimeNs,
  plainTimeNs,
  temporalDurationNs,
  temporalKind,
  typeName,
} from "./value.js";

/** The source type (spec 5.2) of a JavaScript value. */
export function sourceType(x: unknown, path: string): SourceType {
  switch (typeof x) {
    case "undefined":
      return "null";
    case "boolean":
      return "boolean";
    case "number":
    case "bigint":
      return "number";
    case "string":
      return "string";
    case "object":
      break;
    default:
      throw new QuiddityError(`a ${typeof x} is not a source value`, path);
  }
  if (x === null) return "null";
  if (Array.isArray(x)) return "array";
  if (isPlainObject(x) || x instanceof Map || x instanceof ValueMap) return "object";
  if (x instanceof Uint8Array || x instanceof ArrayBuffer || x instanceof DataView) return "bytes";
  if (x instanceof Set || x instanceof ValueSet) return "set";
  if (x instanceof Date || x instanceof Instant) return "instant";
  if (x instanceof Duration) return "duration";
  if (x instanceof Digest || typeName(x) === "URL") return "string";
  const temporal = temporalKind(x);
  if (temporal === "Instant" || temporal === "ZonedDateTime" || temporal === "PlainDateTime") return "instant";
  if (temporal === "Duration" || temporal === "PlainTime") return "duration";
  if (temporal === "PlainDate" || temporal === "PlainYearMonth" || temporal === "PlainMonthDay") {
    return "string";
  }
  if (temporal !== undefined || ArrayBuffer.isView(x)) {
    throw new QuiddityError(`a ${typeName(x)} is not a source value`, path);
  }
  return "object";
}

/** The text of a string source. */
function text(x: unknown): string {
  if (typeof x === "string") return x;
  if (x instanceof Digest) return x.toString();
  if (typeName(x) === "URL") return (x as { href: string }).href;
  return String(x);
}

function bytesOf(x: unknown): Uint8Array {
  if (x instanceof Uint8Array) return x;
  if (x instanceof ArrayBuffer) return new Uint8Array(x);
  const v = x as DataView;
  return new Uint8Array(v.buffer, v.byteOffset, v.byteLength);
}

function instantNs(x: unknown, path: string): bigint {
  if (x instanceof Instant) return x.ns;
  if (x instanceof Date) return dateNs(x, path);
  if (temporalKind(x as object) === "PlainDateTime") return plainDateTimeNs(x as Parameters<typeof plainDateTimeNs>[0]);
  return (x as { epochNanoseconds: bigint }).epochNanoseconds;
}

function durationNs(x: unknown): bigint {
  if (x instanceof Duration) return x.ns;
  if (temporalKind(x as object) === "PlainTime") return plainTimeNs(x as Parameters<typeof plainTimeNs>[0]);
  return temporalDurationNs(x as Parameters<typeof temporalDurationNs>[0]);
}

/** An object source's members. */
interface Members {
  /** The value of the member named `name`, or undefined if there is none. */
  get(name: string): unknown;
  /** The names of its members. */
  names(): string[];
  /** Its entries, keys of any type. */
  entries(): [unknown, unknown][];
}

function members(x: unknown): Members {
  if (x instanceof Map) {
    return {
      get: (name) => x.get(name),
      names: () => [...x.keys()].filter((k): k is string => typeof k === "string"),
      entries: () => [...x.entries()],
    };
  }
  if (x instanceof ValueMap) {
    return {
      get: (name) => x.entries.find(([k]) => k === name)?.[1],
      names: () => x.entries.map(([k]) => k).filter((k): k is string => typeof k === "string"),
      entries: () => x.entries.map(([k, v]) => [k, v]),
    };
  }
  const o = x as Record<string, unknown>;
  // As JSON.stringify reads an object: its own enumerable members, without
  // those whose value is undefined.
  const own = () => Object.entries(o).filter(([, v]) => v !== undefined);
  return {
    get: (name) => (Object.prototype.propertyIsEnumerable.call(o, name) ? o[name] : undefined),
    names: () => own().map(([k]) => k),
    entries: own,
  };
}

function integerOf(x: unknown): bigint | undefined {
  if (typeof x === "bigint") return x;
  if (typeof x === "number" && Number.isInteger(x)) return BigInt(x);
  return undefined;
}

/** Reads source values with the kinds of one identity. */
export class Applier {
  readonly #c: Checker;

  constructor(checker: Checker) {
    this.#c = checker;
  }

  /** The data-model value of source value `s` read with kind `k`. */
  read(k: Kind, s: unknown, path: string): Value {
    while (typeof k === "object" && Object.hasOwn(k, "ref")) k = this.#c.defs[k["ref"]]!;
    const t = sourceType(s, path);
    const name = typeof k === "string" ? k : Object.keys(k)[0]!;
    const opts = typeof k === "string" ? undefined : k[name];
    if (name === "union") return this.#union(opts as Kind[], s, t, path);
    const [native, conv] = this.#c.claims(k);
    if (!native.has(t) && !conv.has(t)) throw new QuiddityError(`${name} does not accept a ${t}`, path);
    const fail = (detail: string): never => {
      throw new QuiddityError(detail, path);
    };
    switch (name) {
      case "any":
        return s === undefined ? null : fromJs(s, path);
      case "null":
        return null;
      case "bool": {
        if (t === "boolean") return s as boolean;
        const word = trimSpace(text(s)).toLowerCase();
        return Object.hasOwn(BOOLS, word) ? BOOLS[word]! : fail(`not a boolean: ${JSON.stringify(s)}`);
      }
      case "integer": {
        if (t === "number") return integerOf(s) ?? fail(`not an integer: ${String(s)}`);
        const x = this.#decimal(text(s), path);
        const r = x === undefined ? undefined : decimalRational(x);
        return r !== undefined && r.d === 1n ? r.n : fail(`not an integer: ${JSON.stringify(s)}`);
      }
      case "number": {
        if (t === "number") return typeof s === "bigint" ? s : normalize(s as number);
        const body = trimSpace(text(s));
        const word = body.toLowerCase().replace(/−/g, "-");
        if (Object.hasOwn(NUMBER_WORDS, word)) return NUMBER_WORDS[word]!;
        if (this.#decimal(body, path) === undefined) fail(`not a number: ${JSON.stringify(s)}`);
        const n = jsonNumber(body.startsWith("+") ? body.slice(1) : body);
        return typeof n === "bigint" ? n : normalize(n);
      }
      case "string":
        return text(s);
      case "uuid":
        return uuid(text(s)) ?? fail(`not a UUID: ${JSON.stringify(s)}`);
      case "path":
        return t === "string" ? text(s) : this.#pathObject(s, path);
      case "bytes":
        return this.#bytes(opts as { text?: string }, s, t, path);
      case "instant":
      case "duration":
        return this.#time(name, opts as { unit?: string }, s, t, path);
      case "list":
      case "set": {
        const items = t === "set" ? setItems(s) : (s as unknown[]);
        const out = items.map((x, i) => this.read(opts as Kind, x, t === "set" ? `${path}[]` : `${path}[${i}]`));
        return name === "list" ? out : new ValueSet(out);
      }
      case "tuple": {
        const kinds = opts as Kind[];
        const items = s as unknown[];
        if (items.length !== kinds.length) fail(`expected ${kinds.length} items, found ${items.length}`);
        return kinds.map((kk, i) => this.read(kk, items[i], `${path}[${i}]`));
      }
      case "map": {
        const m = opts as { key: Kind; value: Kind };
        return new ValueMap(
          members(s).entries().map(([key, x]) => {
            const kp = keyPath(path, key);
            return [this.read(m.key, key, kp), this.read(m.value, x, kp)] as const;
          }),
        );
      }
      case "object":
        return this.object(opts as ObjectKind, members(s), path);
      case "tagged":
        return this.#tagged(opts as TaggedKind, s, t, path);
    }
    throw new Error(`unreachable: ${name}`);
  }

  #decimal(t: string, path: string) {
    try {
      return decimal(trimSpace(t));
    } catch (e) {
      throw new QuiddityError((e as Error).message, path);
    }
  }

  #union(alts: Kind[], s: unknown, t: SourceType, path: string): Value {
    const objects = alts.filter((a) => {
      const r = this.#c.resolve(a);
      return typeof r === "object" && Object.hasOwn(r, "object");
    });
    if (t === "object" && objects.length > 1) {
      const ms = members(s);
      const names = ms.names();
      for (const a of objects) {
        const o = (this.#c.resolve(a) as { object: ObjectKind }).object;
        const required = this.#c.required(o);
        if (names.every((n) => Object.hasOwn(o.fields, n)) && [...required].every((n) => names.includes(n))) {
          return this.read(a, s, path);
        }
      }
      for (const a of alts) {
        if (this.#c.claims(a)[1].has("object")) return this.read(a, s, path);
      }
      throw new QuiddityError("the object fits none of the union's object alternatives", path);
    }
    for (const pick of [0, 1] as const) {
      for (const a of alts) {
        if (this.#c.claims(a)[pick].has(t)) return this.read(a, s, path);
      }
    }
    throw new QuiddityError(`no alternative of the union accepts a ${t}`, path);
  }

  #pathObject(s: unknown, path: string): Value {
    const entries = members(s).entries();
    const platform = entries.length === 1 ? entries[0]![0] : undefined;
    if (platform !== "Unix" && platform !== "Windows") {
      throw new QuiddityError("a path object has exactly one member, Unix or Windows", path);
    }
    const limit = platform === "Unix" ? 255 : 65535;
    const units = entries[0]![1];
    const values = Array.isArray(units) ? units.map(integerOf) : undefined;
    if (values === undefined || !values.every((u) => u !== undefined && u >= 0n && u <= BigInt(limit))) {
      throw new QuiddityError(`${platform} must be an array of integers from 0 to ${limit}`, path);
    }
    const codes = values.map(Number);
    if (platform === "Unix") return Uint8Array.from(codes);
    let s16 = "";
    for (let i = 0; i < codes.length; i += 4096) s16 += String.fromCharCode(...codes.slice(i, i + 4096));
    return wtf8(s16);
  }

  #bytes(opts: { text?: string }, s: unknown, t: SourceType, path: string): Value {
    if (t === "bytes") return bytesOf(s);
    if (t === "array") {
      const values = (s as unknown[]).map(integerOf);
      if (!values.every((b) => b !== undefined && b >= 0n && b <= 255n)) {
        throw new QuiddityError("bytes must be integers from 0 to 255", path);
      }
      return Uint8Array.from(values.map(Number));
    }
    const body = trimSpace(text(s));
    if (opts.text === "hex") return hexBytes(body) ?? fail(`not hex: ${JSON.stringify(s)}`, path);
    const b = base64Bytes(body);
    return typeof b === "string" ? fail(b, path) : b;
  }

  #time(kind: "instant" | "duration", opts: { unit?: string }, s: unknown, t: SourceType, path: string): Value {
    const unit = opts.unit;
    if (t === kind) return kind === "instant" ? instantNs(s, path) : durationNs(s);
    if (t === "number") {
      if (unit === undefined) throw new QuiddityError(`a number for ${kind} needs a declared unit`, path);
      const r: Rational = typeof s === "bigint" ? { n: s, d: 1n } : Number.isInteger(s) ? { n: BigInt(s as number), d: 1n } : finite(s as number, path);
      return exactNanos(r, unit, path);
    }
    if (t === "string") {
      const body = trimSpace(text(s));
      if (unit !== undefined) {
        const x = this.#decimal(body, path);
        if (x !== undefined) return exactNanos(decimalRational(x), unit, path);
      }
      const ns = kind === "instant" ? rfc3339(body) : isoDuration(body);
      return typeof ns === "string" ? fail(ns, path) : ns;
    }
    // An object, as serde writes Rust's SystemTime and Duration.
    const names = kind === "instant" ? ["secs_since_epoch", "nanos_since_epoch"] : ["secs", "nanos"];
    const ms = members(s);
    const have = ms.names();
    if (have.length !== 2 || !names.every((n) => have.includes(n))) {
      throw new QuiddityError(`expected exactly the members ${names.join(" and ")}`, path);
    }
    const secs = integerOf(ms.get(names[0]!));
    const nanos = integerOf(ms.get(names[1]!));
    if (secs === undefined || nanos === undefined) throw new QuiddityError(`${names.join(" and ")} must be integers`, path);
    if (secs < 0n || nanos < 0n || nanos >= 1_000_000_000n) throw new QuiddityError(`${names.join(" and ")} out of range`, path);
    return secs * 1_000_000_000n + nanos;
  }

  /** An object kind applied to a source object; `skip` is a member to leave out (an internal tag). */
  object(o: ObjectKind, s: Members, path: string, skip?: string): Value {
    const values = new Map<string, Value>();
    const entries: [Value, Value][] = [];
    for (const [fname, f] of Object.entries(o.fields)) {
      const member = fname === skip ? undefined : s.get(fname);
      const v = quantize(this.read(f.kind, member, keyPath(path, fname)), f);
      const omit = f.omit ?? [];
      if ((omit.includes("none") && v === null) || (omit.includes("empty") && isEmpty(v))) continue;
      values.set(fname, v);
      entries.push([f.rename ?? fname, v]);
    }
    if (o.order !== undefined) return o.order.map((n) => values.get(n)!);
    return new ValueMap(entries);
  }

  #tagged(t: TaggedKind, s: unknown, st: SourceType, path: string): Value {
    const style = t.style ?? "external";
    const intMode = t.encode_as === "int";
    const variants = t.variants;
    const label = (vname: string): Value => {
      const variant = variants[vname]!;
      return intMode ? BigInt(variant.discriminant!) : (variant.rename ?? vname);
    };
    const variant = (vname: unknown): Variant => {
      if (typeof vname !== "string" || !Object.hasOwn(variants, vname)) {
        throw new QuiddityError(`unknown variant ${display(vname)}`, path);
      }
      return variants[vname]!;
    };
    if (style === "external") {
      if (st === "string") {
        const vname = text(s);
        if (variant(vname).kind !== undefined) throw new QuiddityError(`variant ${JSON.stringify(vname)} has a value`, path);
        return label(vname);
      }
      const entries = members(s).entries();
      if (entries.length !== 1) {
        throw new QuiddityError("an externally tagged value is a string or an object with one member", path);
      }
      const [vname, x] = entries[0]!;
      const kind = variant(vname).kind;
      if (kind === undefined) throw new QuiddityError(`unit variant ${display(vname)} has a value`, path);
      const v = this.read(kind, x, keyPath(path, vname));
      return intMode ? [label(vname as string), v] : new ValueMap([[label(vname as string), v]]);
    }
    const tag = t.tag!;
    const ms = members(s);
    const vname = ms.get(tag);
    if (vname === undefined) throw new QuiddityError(`expected an object with the member ${JSON.stringify(tag)}`, path);
    const kind = variant(vname).kind;
    const tagEntry = [tag, label(vname as string)] as const;
    if (kind === undefined) return new ValueMap([tagEntry]);
    if (style === "internal") {
      const o = (this.#c.resolve(kind) as { object: ObjectKind }).object;
      const result = this.object(o, ms, path, tag) as ValueMap;
      return new ValueMap([...result.entries, tagEntry]);
    }
    const content = t.content!;
    const x = ms.get(content);
    if (x === undefined) throw new QuiddityError(`expected the member ${JSON.stringify(content)}`, path);
    return new ValueMap([tagEntry, [content, this.read(kind, x, keyPath(path, content))]]);
  }
}

interface Field {
  kind: Kind;
  rename?: string;
  omit?: string[];
  quantize?: number | bigint;
  decimals?: number;
}

export interface ObjectKind {
  fields: Record<string, Field>;
  order?: string[];
}

interface Variant {
  kind?: Kind;
  rename?: string;
  discriminant?: number | bigint;
}

interface TaggedKind {
  variants: Record<string, Variant>;
  style?: string;
  tag?: string;
  content?: string;
  encode_as?: string;
}

function setItems(s: unknown): unknown[] {
  return s instanceof ValueSet ? [...s.items] : [...(s as Set<unknown>)];
}

function display(x: unknown): string {
  return typeof x === "string" ? JSON.stringify(x) : String(x);
}

function fail(detail: string, path: string): never {
  throw new QuiddityError(detail, path);
}

function finite(x: number, path: string): Rational {
  if (!Number.isFinite(x)) throw new QuiddityError(`${x} is not a whole number of nanoseconds`, path);
  return floatRational(x);
}

function exactNanos(r: Rational, unit: string, path: string): bigint {
  const n = r.n * UNITS[unit]!;
  if (n % r.d !== 0n) throw new QuiddityError(`the value in ${unit} is not a whole number of nanoseconds`, path);
  return n / r.d;
}

/** A field's value, quantized as its `quantize` or `decimals` says (spec 5.6). */
function quantize(v: Value, f: Field): Value {
  if (v === null) return v;
  if (f.quantize !== undefined) {
    const q = BigInt(f.quantize);
    return floorDiv(v as bigint, q) * q;
  }
  if (f.decimals !== undefined) return quantizeDecimals(v as bigint | number, f.decimals);
  return v;
}
