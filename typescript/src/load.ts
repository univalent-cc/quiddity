/**
 * Reading identities (spec 5.3, 5.9): validation, canonical form, and the
 * source types each kind claims (spec 5.7). Follows
 * `test-vectors/qident.py`, the reference implementation.
 *
 * Identities are held in their canonical JSON form: plain objects, arrays,
 * strings, and integers as numbers (or bigints, beyond 2^53).
 */

import { InvalidIdentity } from "./errors.js";
import { RawNumber, compareCodePoints, jsonText } from "./json.js";

/** A kind in canonical form (spec 5.4): a string, or an object with one member. */
export type Kind = string | { readonly [name: string]: any };

/** The definitions of an identity, by name. */
export type Defs = Readonly<Record<string, Kind>>;

/** The source types a kind accepts: natively, and by conversion. */
export type Claims = readonly [ReadonlySet<SourceType>, ReadonlySet<SourceType>];

export type SourceType =
  | "null" | "boolean" | "number" | "string" | "array" | "object"
  | "bytes" | "set" | "instant" | "duration";

const SCALARS = new Set(["null", "bool", "integer", "number", "string", "uuid", "path", "any"]);
const COMPOUND = new Set([
  "bytes", "instant", "duration", "list", "set", "tuple", "map", "object", "union", "tagged", "ref",
]);
const SOURCE_TYPES: readonly SourceType[] = [
  "null", "boolean", "number", "string", "array", "object", "bytes", "set", "instant", "duration",
];

/** Nanoseconds per unit of `instant` and `duration` (spec 5.4). */
export const UNITS: Readonly<Record<string, bigint>> = { s: 1_000_000_000n, ms: 1_000_000n, us: 1_000n, ns: 1n };

const MIN_I64 = -(2n ** 63n);
const MAX_U64 = 2n ** 64n - 1n;

/** The name of a kind: `"string"`, or the one member of `{"list": …}`. */
export function kindName(k: unknown, where: string): string {
  if (typeof k === "string") {
    if (!SCALARS.has(k)) throw new InvalidIdentity(`unknown kind ${JSON.stringify(k)}`, where);
    return k;
  }
  if (isObject(k)) {
    const names = Object.keys(k);
    if (names.length === 1) {
      const name = names[0]!;
      if (!COMPOUND.has(name)) throw new InvalidIdentity(`unknown kind ${JSON.stringify(name)}`, where);
      return name;
    }
  }
  throw new InvalidIdentity("not a kind: a kind is a string or an object with one member", where);
}

function isObject(x: unknown): x is Record<string, unknown> {
  return typeof x === "object" && x !== null && !Array.isArray(x) && !(x instanceof RawNumber);
}

function members(
  obj: unknown,
  where: string,
  required: readonly string[] = [],
  optional: readonly string[] = [],
): asserts obj is Record<string, unknown> {
  if (!isObject(obj)) throw new InvalidIdentity("expected an object", where);
  for (const m of required) {
    if (!Object.hasOwn(obj, m)) throw new InvalidIdentity(`missing member ${JSON.stringify(m)}`, where);
  }
  for (const m of Object.keys(obj)) {
    if (!required.includes(m) && !optional.includes(m)) {
      throw new InvalidIdentity(`unknown member ${JSON.stringify(m)}`, where);
    }
  }
}

function oneOf(value: unknown, allowed: readonly string[], where: string): string {
  if (typeof value !== "string" || !allowed.includes(value)) {
    throw new InvalidIdentity(`${display(value)} is not one of ${allowed.map((a) => JSON.stringify(a)).join(", ")}`, where);
  }
  return value;
}

function nonEmptyString(value: unknown, where: string): string {
  if (typeof value !== "string" || value === "") throw new InvalidIdentity("expected a non-empty string", where);
  return value;
}

function display(value: unknown): string {
  try {
    return jsonText(value);
  } catch {
    return String(value);
  }
}

/** An integer member: written without a fraction or exponent (spec 5.3). */
function integer(value: unknown, where: string): bigint {
  if (value instanceof RawNumber && value.isIntegerText) return BigInt(value.text);
  if (typeof value === "bigint") return value;
  if (typeof value === "number" && Number.isInteger(value)) return BigInt(value);
  throw new InvalidIdentity(`expected an integer, not ${display(value)}`, where);
}

/** An integer as the JSON form holds it: a number, or a bigint beyond 2^53. */
export function jsonInteger(n: bigint): number | bigint {
  return n >= BigInt(Number.MIN_SAFE_INTEGER) && n <= BigInt(Number.MAX_SAFE_INTEGER) ? Number(n) : n;
}

/** A string that is equal for kinds equal in canonical form. */
export function canonKey(k: unknown): string {
  return jsonText(k, true);
}

// --- Canonical form (spec 5.3) -----------------------------------------------------

/** Validates a kind's syntax and returns it in canonical form. */
export function canonical(k: unknown, where: string): Kind {
  const name = kindName(k, where);
  if (typeof k === "string") return k;
  const v = (k as Record<string, unknown>)[name];
  const at = `${where}.${name}`;
  switch (name) {
    case "bytes": {
      members(v, at, [], ["text"]);
      const text = Object.hasOwn(v, "text") ? oneOf(v["text"], ["base64", "hex"], `${at}.text`) : "base64";
      return { bytes: text === "base64" ? {} : { text } };
    }
    case "instant":
    case "duration": {
      members(v, at, [], ["unit"]);
      return { [name]: Object.hasOwn(v, "unit") ? { unit: oneOf(v["unit"], Object.keys(UNITS), `${at}.unit`) } : {} };
    }
    case "list":
    case "set":
      return { [name]: canonical(v, at) };
    case "tuple": {
      if (!Array.isArray(v) || v.length === 0) throw new InvalidIdentity("a tuple needs a non-empty array of kinds", at);
      return { tuple: v.map((x, i) => canonical(x, `${at}[${i}]`)) };
    }
    case "map": {
      members(v, at, ["key", "value"]);
      return { map: { key: canonical(v["key"], `${at}.key`), value: canonical(v["value"], `${at}.value`) } };
    }
    case "object":
      return { object: canonicalObject(v, at) };
    case "union": {
      if (!Array.isArray(v) || v.length === 0) throw new InvalidIdentity("a union needs a non-empty array of kinds", at);
      // Nested unions are flattened, and equal alternatives merged (spec 5.7).
      const alts = new Map<string, Kind>();
      v.forEach((x, i) => {
        const c = canonical(x, `${at}[${i}]`);
        const flat = typeof c === "object" && Object.hasOwn(c, "union") ? (c["union"] as Kind[]) : [c];
        for (const a of flat) alts.set(canonKey(a), a);
      });
      const ordered = [...alts.keys()].sort(compareCodePoints).map((key) => alts.get(key)!);
      return ordered.length === 1 ? ordered[0]! : { union: ordered };
    }
    case "tagged":
      return { tagged: canonicalTagged(v, at) };
    case "ref":
      return { ref: nonEmptyString(v, at) };
  }
  throw new Error(`unreachable: ${name}`);
}

function canonicalObject(v: unknown, where: string): Record<string, unknown> {
  members(v, where, ["fields"], ["unlisted", "order"]);
  const source = v["fields"];
  if (!isObject(source)) throw new InvalidIdentity("fields must be an object", `${where}.fields`);
  const fields: Record<string, Record<string, unknown>> = {};
  const keys = new Set<string>();
  for (const [fname, f] of Object.entries(source)) {
    const fw = `${where}.fields.${fname}`;
    members(f, fw, ["kind"], ["rename", "omit", "quantize", "decimals"]);
    const out: Record<string, unknown> = { kind: canonical(f["kind"], `${fw}.kind`) };
    if (Object.hasOwn(f, "rename")) {
      const rename = nonEmptyString(f["rename"], `${fw}.rename`);
      if (rename !== fname) out["rename"] = rename;
    }
    const key = (out["rename"] as string | undefined) ?? fname;
    if (keys.has(key)) throw new InvalidIdentity(`two fields have the key ${JSON.stringify(key)}`, fw);
    keys.add(key);
    if (Object.hasOwn(f, "omit")) {
      const omit = f["omit"];
      if (!Array.isArray(omit)) throw new InvalidIdentity("omit must be an array of distinct conditions", `${fw}.omit`);
      const conditions = omit.map((c, i) => oneOf(c, ["none", "empty"], `${fw}.omit[${i}]`));
      if (new Set(conditions).size !== conditions.length) {
        throw new InvalidIdentity("omit must be an array of distinct conditions", `${fw}.omit`);
      }
      if (conditions.length > 0) out["omit"] = conditions.sort();
    }
    if (Object.hasOwn(f, "quantize")) {
      const q = integer(f["quantize"], `${fw}.quantize`);
      if (q < 1n) throw new InvalidIdentity("quantize must be a positive integer", `${fw}.quantize`);
      if (q !== 1n) out["quantize"] = jsonInteger(q);
    }
    if (Object.hasOwn(f, "decimals")) {
      const d = integer(f["decimals"], `${fw}.decimals`);
      if (d < -300n || d > 300n) throw new InvalidIdentity("decimals must be an integer from -300 to 300", `${fw}.decimals`);
      out["decimals"] = Number(d);
    }
    if (Object.hasOwn(f, "quantize") && Object.hasOwn(f, "decimals")) {
      throw new InvalidIdentity("a field has quantize or decimals, not both", fw);
    }
    fields[fname] = out;
  }
  const out: Record<string, unknown> = { fields };
  if (Object.hasOwn(v, "unlisted")) oneOf(v["unlisted"], ["skip"], `${where}.unlisted`);
  if (Object.hasOwn(v, "order")) {
    const order = v["order"];
    const names = Object.keys(fields);
    if (
      !Array.isArray(order) ||
      !order.every((n) => typeof n === "string") ||
      new Set(order).size !== order.length ||
      order.length !== names.length ||
      !order.every((n) => Object.hasOwn(fields, n as string))
    ) {
      throw new InvalidIdentity("order must name every field exactly once", `${where}.order`);
    }
    if (Object.values(fields).some((f) => Object.hasOwn(f, "omit"))) {
      throw new InvalidIdentity("an object with order has a field with omit", where);
    }
    out["order"] = [...order];
  }
  return out;
}

function canonicalTagged(v: unknown, where: string): Record<string, unknown> {
  members(v, where, ["variants"], ["style", "tag", "content", "encode_as"]);
  const style = Object.hasOwn(v, "style") ? oneOf(v["style"], ["external", "internal", "adjacent"], `${where}.style`) : "external";
  const encodeAs = Object.hasOwn(v, "encode_as") ? oneOf(v["encode_as"], ["string", "int"], `${where}.encode_as`) : "string";
  const has = (m: string) => Object.hasOwn(v, m);
  if (style === "external" && (has("tag") || has("content"))) {
    throw new InvalidIdentity("tag and content are not allowed with style external", where);
  }
  let tag: string | undefined;
  let content: string | undefined;
  if (style !== "external") {
    if (!has("tag")) throw new InvalidIdentity(`style ${style} needs tag`, where);
    tag = nonEmptyString(v["tag"], `${where}.tag`);
  }
  if (style === "internal" && has("content")) throw new InvalidIdentity("content is not allowed with style internal", where);
  if (style === "adjacent") {
    if (!has("content")) throw new InvalidIdentity("style adjacent needs content", where);
    content = nonEmptyString(v["content"], `${where}.content`);
    if (content === tag) throw new InvalidIdentity("content must differ from tag", where);
  }
  const variants = v["variants"];
  if (!isObject(variants) || Object.keys(variants).length === 0) {
    throw new InvalidIdentity("variants must be a non-empty object", `${where}.variants`);
  }
  const outVariants: Record<string, Record<string, unknown>> = {};
  const labels = new Set<string>();
  for (const [vname, variant] of Object.entries(variants)) {
    const vw = `${where}.variants.${vname}`;
    members(variant, vw, [], ["kind", "rename", "discriminant"]);
    const out: Record<string, unknown> = {};
    if (Object.hasOwn(variant, "kind")) out["kind"] = canonical(variant["kind"], `${vw}.kind`);
    if (Object.hasOwn(variant, "rename")) {
      const rename = nonEmptyString(variant["rename"], `${vw}.rename`);
      if (rename !== vname) out["rename"] = rename;
    }
    let label: string;
    if (encodeAs === "int") {
      if (!Object.hasOwn(variant, "discriminant")) {
        throw new InvalidIdentity("encode_as int needs an integer discriminant", vw);
      }
      const d = integer(variant["discriminant"], `${vw}.discriminant`);
      if (d < MIN_I64 || d > MAX_U64) throw new InvalidIdentity("a discriminant is a 64-bit integer", `${vw}.discriminant`);
      out["discriminant"] = jsonInteger(d);
      label = `int:${d}`;
    } else {
      if (Object.hasOwn(variant, "discriminant")) {
        throw new InvalidIdentity("discriminant is allowed only with encode_as int", vw);
      }
      label = `str:${(out["rename"] as string | undefined) ?? vname}`;
    }
    if (labels.has(label)) throw new InvalidIdentity(`two variants have the label ${label.slice(4)}`, vw);
    labels.add(label);
    outVariants[vname] = out;
  }
  const out: Record<string, unknown> = { variants: outVariants };
  if (style !== "external") {
    out["style"] = style;
    out["tag"] = tag;
  }
  if (style === "adjacent") out["content"] = content;
  if (encodeAs !== "string") out["encode_as"] = encodeAs;
  return out;
}

/** An identity in canonical form: its root, its definitions and its domain. */
export interface Loaded {
  readonly root: Kind;
  readonly defs: Defs;
  readonly domain: string | undefined;
}

/** Validates an identity's JSON form (spec 5.9) and returns it in canonical form (spec 5.3). */
export function load(doc: unknown): Loaded {
  members(doc, "$", ["root"], ["domain", "definitions"]);
  const domain = Object.hasOwn(doc, "domain") ? nonEmptyString(doc["domain"], "$.domain") : undefined;
  const root = canonical(doc["root"], "$.root");
  const defs: Record<string, Kind> = {};
  if (Object.hasOwn(doc, "definitions")) {
    const source = doc["definitions"];
    if (!isObject(source)) throw new InvalidIdentity("definitions must be an object", "$.definitions");
    for (const [name, k] of Object.entries(source)) {
      nonEmptyString(name, "$.definitions");
      defs[name] = canonical(k, `$.definitions.${name}`);
    }
  }
  return build(root, defs, domain);
}

/** Checks kinds already in canonical form as one identity, and drops the definitions it does not reach. */
export function build(root: Kind, defs: Record<string, Kind>, domain: string | undefined): Loaded {
  if (domain !== undefined) nonEmptyString(domain, "$.domain");
  const checker = new Checker(defs);
  checker.walk(root, "$.root");
  for (const [name, k] of Object.entries(defs)) checker.walk(k, `$.definitions.${name}`);
  // Definitions the root does not reach are not part of the identity.
  const reached = new Set<string>();
  reach(root, defs, reached);
  const kept: Record<string, Kind> = {};
  for (const name of Object.keys(defs).sort(compareCodePoints)) {
    if (reached.has(name)) kept[name] = defs[name]!;
  }
  return { root, defs: kept, domain };
}

function reach(k: Kind, defs: Defs, reached: Set<string>): void {
  if (typeof k === "string") return;
  const [name, v] = Object.entries(k)[0]!;
  switch (name) {
    case "ref":
      if (!reached.has(v)) {
        reached.add(v);
        reach(defs[v]!, defs, reached);
      }
      break;
    case "list":
    case "set":
      reach(v, defs, reached);
      break;
    case "tuple":
    case "union":
      for (const x of v) reach(x, defs, reached);
      break;
    case "map":
      reach(v.key, defs, reached);
      reach(v.value, defs, reached);
      break;
    case "object":
      for (const f of Object.values(v.fields) as { kind: Kind }[]) reach(f.kind, defs, reached);
      break;
    case "tagged":
      for (const variant of Object.values(v.variants) as { kind?: Kind }[]) {
        if (variant.kind !== undefined) reach(variant.kind, defs, reached);
      }
      break;
  }
}

// --- Claims and the checks that need the whole identity (spec 5.7, 5.9) -------------

const TABLE: Record<string, readonly [readonly SourceType[], readonly SourceType[]]> = {
  null: [["null"], []],
  bool: [["boolean"], ["string"]],
  integer: [["number"], ["string"]],
  number: [["number"], ["string"]],
  string: [["string"], []],
  uuid: [["string"], []],
  path: [["string"], ["object"]],
  any: [SOURCE_TYPES, []],
  list: [["array"], []],
  set: [["array", "set"], []],
  tuple: [["array"], []],
  map: [["object"], []],
  object: [["object"], []],
  bytes: [["bytes"], ["array", "string"]],
};

const QUANTIZED: Record<string, readonly string[]> = {
  quantize: ["integer", "instant", "duration"],
  decimals: ["number"],
};

/** The claims of kinds and the checks of spec 5.9, for the kinds of one identity. */
export class Checker {
  readonly defs: Defs;
  readonly #claims = new Map<Kind, Claims>();
  readonly #required = new Map<object, ReadonlySet<string>>();

  constructor(defs: Defs) {
    this.defs = defs;
  }

  /** A kind with references followed. */
  resolve(k: Kind, where = "$"): Kind {
    const seen = new Set<string>();
    while (typeof k === "object" && Object.hasOwn(k, "ref")) {
      const ref = k["ref"] as string;
      if (seen.has(ref)) throw new InvalidIdentity(`a cycle of references through ${JSON.stringify(ref)}`, where);
      seen.add(ref);
      const next = this.defs[ref];
      if (next === undefined) throw new InvalidIdentity(`ref to undefined ${JSON.stringify(ref)}`, where);
      k = next;
    }
    return k;
  }

  /** The source types `k` accepts: natively, and by conversion. */
  claims(k: Kind, where = "$", visiting: readonly string[] = []): Claims {
    const found = this.#claims.get(k);
    if (found !== undefined) return found;
    const c = this.#computeClaims(k, where, visiting);
    if (typeof k === "object") this.#claims.set(k, c);
    return c;
  }

  #computeClaims(k: Kind, where: string, visiting: readonly string[]): Claims {
    const name = kindName(k, where);
    if (typeof k === "string") return toClaims(TABLE[name]!);
    const v = k[name];
    if (name === "ref") {
      if (visiting.includes(v)) {
        throw new InvalidIdentity(`a cycle of references through ${JSON.stringify(v)} without structure`, where);
      }
      const def = this.defs[v];
      if (def === undefined) throw new InvalidIdentity(`ref to undefined ${JSON.stringify(v)}`, where);
      return this.claims(def, where, [...visiting, v]);
    }
    if (name === "union") return this.unionClaims(v, where, visiting);
    if (name in TABLE) return toClaims(TABLE[name]!);
    if (name === "instant" || name === "duration") {
      const conv: SourceType[] = ["string", "object"];
      if (Object.hasOwn(v, "unit")) conv.push("number");
      return [new Set([name]), new Set(conv)];
    }
    if (name === "tagged") {
      if ((v.style ?? "external") !== "external") return [new Set(["object"]), new Set()];
      const native = new Set<SourceType>();
      for (const variant of Object.values(v.variants) as { kind?: Kind }[]) {
        native.add(variant.kind !== undefined ? "object" : "string");
      }
      return [native, new Set()];
    }
    throw new Error(`unreachable: ${name}`);
  }

  /** A union's claims, checking that it is valid (spec 5.7). */
  unionClaims(alts: readonly Kind[], where: string, visiting: readonly string[] = []): Claims {
    const native = new Set<SourceType>();
    const conv = new Set<SourceType>();
    // Claimed first, with `visiting`, so that a cycle of references is found
    // before the object alternatives are looked for.
    const claimed = alts.map((a) => this.claims(a, where, visiting));
    const objects = this.objectAlternatives(alts, where);
    for (const [n, c] of claimed) {
      let overlap = [...n].filter((t) => native.has(t));
      if (overlap.length === 1 && overlap[0] === "object" && objects !== undefined) overlap = []; // told apart by keys
      if (overlap.length > 0) {
        throw new InvalidIdentity(`two union alternatives accept ${overlap.join(", ")} natively`, where);
      }
      const both = [...c].filter((t) => conv.has(t));
      if (both.length > 0) throw new InvalidIdentity(`two union alternatives accept ${both.join(", ")} by conversion`, where);
      for (const t of n) native.add(t);
      for (const t of c) conv.add(t);
    }
    if (objects !== undefined && objects.length > 1) {
      for (let i = 0; i < objects.length; i++) {
        for (let j = i + 1; j < objects.length; j++) {
          const a = objects[i]!, b = objects[j]!;
          const fa = Object.keys(a.fields), fb = Object.keys(b.fields);
          if ([...this.required(a)].every((f) => fb.includes(f)) && [...this.required(b)].every((f) => fa.includes(f))) {
            throw new InvalidIdentity(
              `two object alternatives of a union could both fit one object: [${fa.join(", ")}] and [${fb.join(", ")}]`,
              where,
            );
          }
        }
      }
    }
    return [native, conv];
  }

  /**
   * The object kinds among a union's alternatives that accept objects
   * natively, if they are all object kinds (or refs to them); else undefined.
   */
  objectAlternatives(alts: readonly Kind[], where = "$"): { fields: Record<string, { kind: Kind }> }[] | undefined {
    const objs: { fields: Record<string, { kind: Kind }> }[] = [];
    for (const a of alts) {
      const r = this.resolve(a, where);
      if (typeof r === "object" && Object.hasOwn(r, "object")) objs.push(r["object"]);
      else if (this.claims(a, where)[0].has("object")) return undefined;
    }
    return objs;
  }

  /** The fields of object kind `o` whose kinds do not accept null. */
  required(o: { fields: Record<string, { kind: Kind }> }): ReadonlySet<string> {
    let found = this.#required.get(o);
    if (found === undefined) {
      found = new Set(Object.entries(o.fields).filter(([, f]) => !this.claims(f.kind)[0].has("null")).map(([n]) => n));
      this.#required.set(o, found);
    }
    return found;
  }

  /** The checks of spec 5.9 that need the whole identity. */
  walk(k: Kind, where: string): void {
    const name = kindName(k, where);
    if (typeof k === "string") return;
    const v = k[name];
    const at = `${where}.${name}`;
    switch (name) {
      case "ref":
        if (this.defs[v] === undefined) throw new InvalidIdentity(`ref to undefined ${JSON.stringify(v)}`, at);
        this.claims(k, at); // detects cycles without structure
        break;
      case "list":
      case "set":
        this.walk(v, at);
        break;
      case "tuple":
        v.forEach((x: Kind, i: number) => this.walk(x, `${at}[${i}]`));
        break;
      case "map": {
        this.walk(v.key, `${at}.key`);
        this.walk(v.value, `${at}.value`);
        const r = this.resolve(v.key, at);
        const keyAlts: Kind[] = typeof r === "object" && Object.hasOwn(r, "union") ? r["union"] : [v.key];
        const accepting = keyAlts.filter((a) => {
          const [n, c] = this.claims(a, at);
          return n.has("string") || c.has("string");
        });
        if (accepting.length === 0) throw new InvalidIdentity("a map's key kind must accept strings", `${at}.key`);
        if (accepting.length > 1) {
          throw new InvalidIdentity("a map's key union has two alternatives that accept strings", `${at}.key`);
        }
        break;
      }
      case "object":
        for (const [fname, f] of Object.entries(v.fields) as [string, Record<string, unknown>][]) {
          const fw = `${at}.fields.${fname}`;
          this.walk(f["kind"] as Kind, `${fw}.kind`);
          for (const [option, kinds] of Object.entries(QUANTIZED)) {
            if (Object.hasOwn(f, option) && !kinds.includes(this.quantizedKind(f["kind"] as Kind, fw) ?? "")) {
              throw new InvalidIdentity(`${option} does not apply to the field's kind`, fw);
            }
          }
        }
        break;
      case "union":
        v.forEach((a: Kind, i: number) => this.walk(a, `${at}[${i}]`));
        this.unionClaims(v, at);
        break;
      case "tagged":
        for (const [vname, variant] of Object.entries(v.variants) as [string, { kind?: Kind }][]) {
          if (variant.kind !== undefined) this.walk(variant.kind, `${at}.variants.${vname}.kind`);
        }
        if (v.style === "internal") {
          for (const [vname, variant] of Object.entries(v.variants) as [string, { kind?: Kind }][]) {
            if (variant.kind === undefined) continue;
            const vw = `${at}.variants.${vname}`;
            const obj = this.resolve(variant.kind, vw);
            if (!(typeof obj === "object" && Object.hasOwn(obj, "object"))) {
              throw new InvalidIdentity("an internally tagged variant's kind is not an object", vw);
            }
            const o = obj["object"];
            if (Object.hasOwn(o, "order")) throw new InvalidIdentity("an internally tagged variant's object has order", vw);
            for (const [fname, f] of Object.entries(o.fields) as [string, { rename?: string }][]) {
              if (v.tag === fname || v.tag === f.rename) {
                throw new InvalidIdentity(`the variant has a field named ${JSON.stringify(v.tag)}, its tag`, vw);
              }
            }
          }
        }
        break;
    }
  }

  /**
   * The name of the kind a quantization applies to: the field's kind, or the
   * one alternative besides null of a union; references followed.
   */
  quantizedKind(k: Kind, where: string): string | undefined {
    let r = this.resolve(k, where);
    if (typeof r === "object" && Object.hasOwn(r, "union")) {
      const others = (r["union"] as Kind[]).filter((a) => this.resolve(a, where) !== "null");
      if (others.length !== 1) return undefined;
      r = this.resolve(others[0]!, where);
    }
    return kindName(r, where);
  }
}

function toClaims(t: readonly [readonly SourceType[], readonly SourceType[]]): Claims {
  return [new Set(t[0]), new Set(t[1])];
}
