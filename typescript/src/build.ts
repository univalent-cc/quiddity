/**
 * Kinds built in code (spec 5.4), with the TypeScript types of the values
 * they describe inferred from them, and `objectFor`, which checks an object
 * kind against a type.
 *
 * Each kind has two type parameters: `Natural`, the type of the values it
 * describes as JavaScript holds them, and `Accepted`, the type of the
 * values it reads, which is wider where a kind converts (spec 5.5): an
 * `integer` describes `number | bigint` and also reads `"5"`.
 */

import { BUILD, BUILD_FIELD, type BuildContext, type Buildable, type BuildableField } from "./context.js";
import type { Digest } from "./digest.js";
import { InvalidIdentity } from "./errors.js";
import { Identity, type IdentityJson } from "./identity.js";
import type { Kind as KindJson } from "./load.js";
import type { Duration, Instant, Value, ValueMap, ValueSet } from "./model.js";
import { READER, type ReadsInstances } from "./value.js";

/**
 * A kind built in code. `Natural` is the type of the values it describes,
 * `Accepted` the type of the values it reads.
 */
export class Kind<Natural = unknown, Accepted = Natural> implements Buildable, BuildableField {
  declare readonly __natural?: Natural;
  declare readonly __accepts?: (value: Accepted) => void;

  readonly #build: (ctx: BuildContext) => KindJson;
  readonly #name: string | undefined;
  #identity: Identity<Natural, Accepted> | undefined;

  constructor(build: (ctx: BuildContext) => KindJson, name?: string) {
    this.#build = build;
    this.#name = name;
  }

  /**
   * The kind as the definition `name`: identities that reach it hold it once,
   * in their `definitions`, and refer to it by name.
   */
  named(name: string): Kind<Natural, Accepted> {
    if (typeof name !== "string" || name === "") throw new InvalidIdentity("a name is a non-empty string");
    return new Kind(this.#build, name);
  }

  /** The identity of this kind's values, without a domain. */
  get identity(): Identity<Natural, Accepted> {
    this.#identity ??= new Identity(this);
    return this.#identity;
  }

  /** The identity of this kind's values, in the domain `domain`. */
  inDomain(domain: string): Identity<Natural, Accepted> {
    return new Identity(this, { domain });
  }

  /** The digest of `value` under this kind, without a domain. */
  digest(value: Accepted): Digest {
    return this.identity.digest(value);
  }

  /** The data-model value of `value` under this kind. */
  read(value: Accepted): Value {
    return this.identity.read(value);
  }

  /** The identity's JSON form. */
  toJson(): IdentityJson {
    return this.identity.toJson();
  }

  /** As a field: its entry left out when its value is null (`"none"`), or empty (`"empty"`). */
  omit(...conditions: OmitCondition[]): Field<Natural, Accepted> {
    return new Field<Natural, Accepted>(this, {}).omit(...conditions);
  }

  /** As a field: its key in the result, if not its name. */
  rename(key: string): Field<Natural, Accepted> {
    return new Field<Natural, Accepted>(this, {}).rename(key);
  }

  /** As a field: its value, an integer, instant or duration, rounded down to a multiple of `step`. */
  quantize(step: number | bigint): Field<Natural, Accepted> {
    return new Field<Natural, Accepted>(this, {}).quantize(step);
  }

  /** As a field: its value, a number, rounded down to a multiple of `10^-places`. */
  decimals(places: number): Field<Natural, Accepted> {
    return new Field<Natural, Accepted>(this, {}).decimals(places);
  }

  [BUILD](ctx: BuildContext): KindJson {
    return this.#name === undefined ? this.#build(ctx) : ctx.define(this.#name, () => this.#build(ctx));
  }

  [BUILD_FIELD](ctx: BuildContext): Record<string, unknown> {
    return { kind: this[BUILD](ctx) };
  }

  [READER](): ReadsInstances {
    return this.identity[READER]();
  }
}

/** A condition under which a field's entry is left out (spec 5.6). */
export type OmitCondition = "none" | "empty";

interface FieldOptions {
  rename?: string;
  omit?: OmitCondition[];
  quantize?: number | bigint;
  decimals?: number;
}

/** A field of an object kind: a kind, with the options of spec 5.6. */
export class Field<Natural = unknown, Accepted = Natural> implements BuildableField {
  declare readonly __natural?: Natural;
  declare readonly __accepts?: (value: Accepted) => void;

  readonly #kind: Buildable;
  readonly #options: Readonly<FieldOptions>;

  constructor(kind: Buildable, options: Readonly<FieldOptions>) {
    this.#kind = kind;
    this.#options = options;
  }

  omit(...conditions: OmitCondition[]): Field<Natural, Accepted> {
    return new Field(this.#kind, { ...this.#options, omit: [...(this.#options.omit ?? []), ...conditions] });
  }

  rename(key: string): Field<Natural, Accepted> {
    return new Field(this.#kind, { ...this.#options, rename: key });
  }

  quantize(step: number | bigint): Field<Natural, Accepted> {
    return new Field(this.#kind, { ...this.#options, quantize: step });
  }

  decimals(places: number): Field<Natural, Accepted> {
    return new Field(this.#kind, { ...this.#options, decimals: places });
  }

  [BUILD_FIELD](ctx: BuildContext): Record<string, unknown> {
    return { kind: this.#kind[BUILD](ctx), ...this.#options };
  }
}

/** A field of a type that the identity leaves out, in {@link objectFor}. */
export class Skip {
  declare readonly __skip: true;
}

const SKIP = new Skip();

/** In {@link objectFor}: the field does not count. */
export function skip(): Skip {
  return SKIP;
}

/** A kind, or an identity (nested as its root). */
export type KindLike = Kind<any, any> | Identity<any, any>;

/** What can be a field: a kind, a field, or an identity. */
export type FieldKind = KindLike | Field<any, any>;

/** The type of the values a kind or field describes. */
export type Infer<K> =
  K extends Kind<infer N, any> ? N : K extends Field<infer N, any> ? N : K extends Identity<infer N, any> ? N : never;

/** The type of the values a kind or field reads. */
export type Accepts<K> =
  K extends Kind<any, infer A> ? A : K extends Field<any, infer A> ? A : K extends Identity<any, infer A> ? A : never;

type Flatten<T> = { [K in keyof T]: T[K] } & {};

/** An object with the natural types of fields `F`: a field whose kind accepts null is optional. */
export type ObjectOf<F> = Flatten<
  { -readonly [K in keyof F as null extends Infer<F[K]> ? never : K]: Infer<F[K]> } & {
    -readonly [K in keyof F as null extends Infer<F[K]> ? K : never]?: Infer<F[K]>;
  }
>;

/** An object with the accepted types of fields `F`. */
export type ObjectAccepting<F> = Flatten<
  { [K in keyof F as undefined extends Accepts<F[K]> ? never : K]: Accepts<F[K]> } & {
    [K in keyof F as undefined extends Accepts<F[K]> ? K : never]?: Accepts<F[K]>;
  }
>;

function buildKind(k: KindLike, ctx: BuildContext): KindJson {
  if (BUILD in k) return k[BUILD](ctx);
  throw new InvalidIdentity("a field's options are not a kind; use them only for fields of an object");
}

function scalar<N, A = N>(name: string): () => Kind<N, A> {
  const k = new Kind<N, A>(() => name);
  return () => k;
}

// --- Scalars -------------------------------------------------------------------------

const nullKind: () => Kind<null, null | undefined> = scalar("null");
export { nullKind as null };

/** A boolean; also reads `"yes"`, `"off"` and the other words of spec 5.5. */
export const bool: () => Kind<boolean, boolean | string> = scalar("bool");

/** An integer; also reads decimal text whose value is an integer, such as `"5"` or `"1e3"`. */
export const integer: () => Kind<number | bigint, number | bigint | string> = scalar("integer");

/** A number: an integer or a float; also reads decimal text and the words for NaN and the infinities. */
export const number: () => Kind<number | bigint, number | bigint | string> = scalar("number");

/** A string: text only, never a number rendered as text. */
export const string: () => Kind<string, string> = scalar("string");

/** A UUID, in any of the forms of spec 5.5, as its lowercase hyphenated form. */
export const uuid: () => Kind<string, string> = scalar("uuid");

/** A file system path: text, or an object as serde writes Rust's `OsString`. */
export const path: () => Kind<string, string | { Unix: readonly number[] } | { Windows: readonly number[] }> = scalar("path");

/** Any value, as it is (spec 2.7 for JSON). */
export const any: () => Kind<unknown, unknown> = scalar("any");

/** Bytes: a `Uint8Array`, an array of integers, or text in base64 (the default) or hex. */
export function bytes(options?: { text?: "base64" | "hex" }): Kind<Uint8Array, Uint8Array | ArrayBuffer | readonly number[] | string> {
  const text = options?.text;
  return new Kind(() => ({ bytes: text === undefined ? {} : { text } }));
}

/** The unit of a number read as an instant or a duration. */
export type TimeUnit = "s" | "ms" | "us" | "ns";

/** A `Temporal.Instant` or `Temporal.ZonedDateTime`, as far as quiddity reads it. */
export interface TemporalInstantLike {
  readonly epochNanoseconds: bigint;
}

/** A `Temporal.PlainDateTime`, as far as quiddity reads it. */
export interface TemporalPlainDateTimeLike {
  toZonedDateTime(timeZone: string): TemporalInstantLike;
}

/** What `instant` reads, besides a number with a unit. */
export type InstantSource =
  | Date
  | Instant
  | TemporalInstantLike
  | TemporalPlainDateTimeLike
  | string
  | { secs_since_epoch: number | bigint; nanos_since_epoch: number | bigint };

/** What `duration` reads, besides a number with a unit. */
export type DurationSource = Duration | string | { secs: number | bigint; nanos: number | bigint } | object;

/**
 * An instant (spec 2.6): a `Date`, an `Instant`, a Temporal instant or
 * plain date-time, RFC 3339 text (UTC without an offset), or, with `unit`,
 * a number of units since the epoch.
 */
export function instant(): Kind<Date, InstantSource>;
export function instant(options: { unit: TimeUnit }): Kind<Date, InstantSource | number | bigint>;
export function instant(options?: { unit?: TimeUnit }): Kind<Date, InstantSource | number | bigint> {
  const unit = options?.unit;
  return new Kind(() => ({ instant: unit === undefined ? {} : { unit } }));
}

/**
 * A duration (spec 2.6): a `Duration`, a `Temporal.Duration`, a
 * `Temporal.PlainTime` (the time since midnight), ISO 8601 text such as
 * `"PT1H30M"`, or, with `unit`, a number of units.
 */
export function duration(): Kind<Duration | string, DurationSource>;
export function duration(options: { unit: TimeUnit }): Kind<Duration | string, DurationSource | number | bigint>;
export function duration(options?: { unit?: TimeUnit }): Kind<Duration | string, DurationSource | number | bigint> {
  const unit = options?.unit;
  return new Kind(() => ({ duration: unit === undefined ? {} : { unit } }));
}

// --- Collections ---------------------------------------------------------------------

/** A list: an array, read in order. */
export function list<K extends KindLike>(item: K): Kind<Infer<K>[], readonly Accepts<K>[]> {
  return new Kind((ctx) => ({ list: buildKind(item, ctx) }));
}

/** A set: an array or a `Set`, whose order does not count. */
export function set<K extends KindLike>(item: K): Kind<Set<Infer<K>>, ReadonlySet<Accepts<K>> | readonly Accepts<K>[] | ValueSet> {
  return new Kind((ctx) => ({ set: buildKind(item, ctx) }));
}

/** A tuple: an array with exactly one item for each kind, read with the kind in the same position. */
export function tuple<const K extends readonly KindLike[]>(
  ...items: K
): Kind<{ -readonly [I in keyof K]: Infer<K[I]> }, { readonly [I in keyof K]: Accepts<K[I]> }> {
  return new Kind((ctx) => ({ tuple: items.map((k) => buildKind(k, ctx)) }));
}

/** A map: an object, `Map` or `ValueMap`, its keys read with `key` and its values with `value`. */
export function map<K extends KindLike, V extends KindLike>(
  key: K,
  value: V,
): Kind<
  Infer<K> extends string ? Record<string, Infer<V>> : Map<Infer<K>, Infer<V>>,
  Readonly<Record<string, Accepts<V>>> | ReadonlyMap<Accepts<K>, Accepts<V>> | ValueMap
> {
  return new Kind((ctx) => ({ map: { key: buildKind(key, ctx), value: buildKind(value, ctx) } }));
}

// --- Objects -------------------------------------------------------------------------

/** The options of an object kind (spec 5.6). */
export interface ObjectOptions<Name extends string = string> {
  /** The value is the list of the fields' values in this order, rather than a map. */
  order?: readonly Name[];
}

function objectKind(fields: Record<string, FieldKind | Skip>, options: ObjectOptions | undefined): (ctx: BuildContext) => KindJson {
  return (ctx) => {
    const out: Record<string, unknown> = {};
    for (const [name, f] of Object.entries(fields)) {
      if (f instanceof Skip) continue;
      Object.defineProperty(out, name, { value: f[BUILD_FIELD](ctx), enumerable: true, writable: true, configurable: true });
    }
    const object: Record<string, unknown> = { fields: out };
    if (options?.order !== undefined) object["order"] = [...options.order];
    return { object };
  };
}

/**
 * An object (spec 5.6): the fields named here count, read from the source
 * object's members of the same names; other members are left out. A field
 * whose kind accepts null is optional: a missing member is read as null.
 *
 * ```ts
 * import * as q from "quiddity";
 *
 * const Book = q.object({ title: q.string(), pages: q.integer() });
 * type Book = q.Infer<typeof Book>; // { title: string; pages: number | bigint }
 * ```
 */
export function object<const F extends Record<string, FieldKind>>(
  fields: F,
  options?: ObjectOptions<keyof F & string>,
): Kind<ObjectOf<F>, ObjectAccepting<F>> {
  return new Kind(objectKind(fields, options));
}

type DataKeys<T> = { [K in keyof T]-?: T[K] extends (...args: any[]) => any ? never : K }[keyof T];

/** The type of the member `K` of `T`, with `undefined` if it is optional. */
type MemberValue<T, K extends keyof T> = {} extends Pick<T, K> ? T[K] | undefined : T[K];

/** A field that reads values of type `V`, or {@link Skip}. */
export type FieldFor<V> = Kind<unknown, V> | Field<unknown, V> | Identity<unknown, V> | Skip;

/** The fields of an object kind for the type `T`: one for each of its members that is not a method. */
export type FieldsFor<T> = { [K in keyof T as K extends DataKeys<T> ? K : never]-?: FieldFor<MemberValue<T, K>> };

/**
 * An object kind for the type `T`, checked by the compiler: it names every
 * member of `T` that is not a method, each with a kind that reads the
 * member's type or with {@link skip}, and no other. A member added to `T`
 * is an error until its field is.
 *
 * ```ts
 * import * as q from "quiddity";
 *
 * interface Book {
 *   title: string;
 *   pages: number;
 *   genres?: string[];
 *   lastOpened: Date;
 * }
 *
 * const BookIdentity = q.objectFor<Book>({
 *   title: q.string(),
 *   pages: q.integer(),
 *   genres: q.optional(q.set(q.string())).omit("none", "empty"),
 *   lastOpened: q.skip(),
 * });
 * ```
 */
export function objectFor<T>(fields: FieldsFor<T>, options?: ObjectOptions<DataKeys<T> & string>): Kind<T, T> {
  return new Kind(objectKind(fields as Record<string, FieldKind | Skip>, options));
}

// --- Unions --------------------------------------------------------------------------

/** A union (spec 5.7): a value that one of the kinds accepts, read with that kind. */
export function union<const K extends readonly KindLike[]>(...kinds: K): Kind<Infer<K[number]>, Accepts<K[number]>> {
  return new Kind((ctx) => ({ union: kinds.map((k) => buildKind(k, ctx)) }));
}

/** A value of `kind`, or null: `union(null(), kind)`. A missing member reads as null. */
export function optional<K extends KindLike>(kind: K): Kind<Infer<K> | null, Accepts<K> | null | undefined> {
  return union(nullKind(), kind) as Kind<Infer<K> | null, Accepts<K> | null | undefined>;
}

/** A variant of a tagged union, with options. */
export class Variant {
  readonly kind: KindLike | null;
  readonly rename: string | undefined;
  readonly discriminant: number | bigint | undefined;

  constructor(kind: KindLike | null, options?: { rename?: string; discriminant?: number | bigint }) {
    this.kind = kind;
    this.rename = options?.rename;
    this.discriminant = options?.discriminant;
  }
}

/** A variant of a tagged union with a rename or a discriminant; `kind` is null for a unit variant. */
export function variant(kind: KindLike | null, options: { rename?: string; discriminant?: number | bigint }): Variant {
  return new Variant(kind, options);
}

/** The options of a tagged union (spec 5.8). */
export interface TaggedOptions {
  style?: "external" | "internal" | "adjacent";
  tag?: string;
  content?: string;
  encodeAs?: "string" | "int";
}

/**
 * A tagged union (spec 5.8): one of the named variants, in one of the forms
 * serde writes Rust enums in. Each variant is a kind, null for a unit
 * variant, or a {@link variant} with options.
 */
export function tagged(variants: Record<string, KindLike | Variant | null>, options?: TaggedOptions): Kind<unknown, unknown> {
  return new Kind((ctx) => {
    const out: Record<string, unknown> = {};
    for (const [name, v] of Object.entries(variants)) {
      const spec = v instanceof Variant ? v : new Variant(v);
      const entry: Record<string, unknown> = {};
      if (spec.kind !== null) entry["kind"] = buildKind(spec.kind, ctx);
      if (spec.rename !== undefined) entry["rename"] = spec.rename;
      if (spec.discriminant !== undefined) entry["discriminant"] = spec.discriminant;
      Object.defineProperty(out, name, { value: entry, enumerable: true, writable: true, configurable: true });
    }
    const t: Record<string, unknown> = { variants: out };
    if (options?.style !== undefined) t["style"] = options.style;
    if (options?.tag !== undefined) t["tag"] = options.tag;
    if (options?.content !== undefined) t["content"] = options.content;
    if (options?.encodeAs !== undefined) t["encode_as"] = options.encodeAs;
    return { tagged: t };
  });
}

/** A reference to the definition `name`, for recursive kinds: `q.list(q.ref("Node"))`. */
export function ref<N = unknown, A = N>(name: string): Kind<N, A> {
  return new Kind(() => ({ ref: name }));
}
