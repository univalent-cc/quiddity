/**
 * Stable, configurable digests that capture the essence of values.
 *
 * A digest is the same in every language that implements quiddity, on every
 * platform, and in every release. Which parts of a value count is an
 * *identity*: built in code with the builders here (`q.object`,
 * `q.objectFor<T>`, …), or read from its JSON form, which every
 * implementation reads and writes.
 *
 * ```ts
 * import * as q from "quiddity";
 *
 * String(q.digest({ title: "Middlemarch", pages: 880 }));
 * // "sha256:…", the same as {pages: 880, title: "Middlemarch"} in any language
 * ```
 *
 * The encoding is specified in `QUIDDITY_SPEC.md`, in the repository at
 * https://github.com/univalent-cc/quiddity.
 *
 * @module
 */

import { Digest } from "./digest.js";
import { asIdentity } from "./identity.js";
import { parseJson as parseJsonValue } from "./json.js";
import { encode } from "./model.js";
import { fromJs, staticIdentity } from "./value.js";

export { Digest } from "./digest.js";
export { InvalidIdentity, QuiddityError } from "./errors.js";
export { Identity, type IdentityJson, type KindJson } from "./identity.js";
export { type Json, parseJson } from "./json.js";
export { Duration, Instant, type Value, ValueMap, ValueSet } from "./model.js";
export { identity } from "./value.js";
export {
  type Accepts,
  type DurationSource,
  Field,
  type FieldFor,
  type FieldKind,
  type FieldsFor,
  type Infer,
  type InstantSource,
  Kind,
  type KindLike,
  type ObjectAccepting,
  type ObjectOf,
  type ObjectOptions,
  type OmitCondition,
  Skip,
  type TaggedOptions,
  type TemporalInstantLike,
  type TimeUnit,
  Variant,
  any,
  bool,
  bytes,
  duration,
  instant,
  integer,
  list,
  map,
  null,
  number,
  object,
  objectFor,
  optional,
  path,
  ref,
  set,
  skip,
  string,
  tagged,
  tuple,
  union,
  uuid,
  variant,
} from "./build.js";

/** An identity, or a kind built in code, which is digested as an identity without a domain. */
export type IdentityLike = import("./identity.js").Identity<any, any> | import("./build.js").KindLike;

/**
 * The digest of `value`: under `identity` if one is given, or else as its
 * data-model value, read as its JSON form would be, except where the data
 * model says more than JSON: a `Date` is an instant, the same in any time
 * zone; a `Set` is a set, whose order does not count; a `Uint8Array` is
 * bytes; a `bigint` an exact integer; and `Map` keys keep their types. A
 * class instance is read under its class's static `[quiddity.identity]`, in
 * its domain. Throws a {@link QuiddityError} if the value has no digest.
 */
export function digest(value: unknown, identity?: IdentityLike): Digest {
  if (identity !== undefined) return asIdentity(identity).digest(value);
  const d = Digest.ofEncoding(encode(fromJs(value)));
  const domain = typeof value === "object" && value !== null ? staticIdentity(value)?.instanceDomain : undefined;
  return domain !== undefined ? d.inDomain(domain) : d;
}

/**
 * The encoding (spec 3) of `value`, under `identity` as {@link digest} takes
 * it: the bytes its digest hashes, for tests and other implementations.
 */
export function encoding(value: unknown, identity?: IdentityLike): Uint8Array {
  if (identity !== undefined) return asIdentity(identity).encoding(value);
  return encode(fromJs(value));
}

/**
 * The digest of JSON text, read as spec 2.7 maps it (see {@link parseJson}),
 * under `identity` if one is given.
 */
export function digestJson(text: string, identity?: IdentityLike): Digest {
  return digest(parseJsonValue(text), identity);
}
