/** Identities as data (spec 5): `Identity`. */

import { Applier, type ObjectKind } from "./apply.js";
import { BUILD, BUILD_FIELD, BuildContext, type Buildable, type BuildableField } from "./context.js";
import { Digest } from "./digest.js";
import { InvalidIdentity, QuiddityError } from "./errors.js";
import { copyJson, jsonText, parseRawJson } from "./json.js";
import { Checker, type Defs, type Kind, build, canonKey, canonical, load } from "./load.js";
import { IDENTITY_OF_IDENTITIES } from "./meta.js";
import { type Value, encode } from "./model.js";
import { READER, type ReadsInstances, identity as identitySymbol } from "./value.js";

/** A kind in an identity's JSON form (spec 5.4). */
export type KindJson = string | { readonly [name: string]: unknown };

/** An identity's JSON form (spec 5.3). */
export interface IdentityJson {
  root: KindJson;
  domain?: string;
  definitions?: Record<string, KindJson>;
}

const MAKE: unique symbol = Symbol("quiddity.make");

/**
 * An identity as data (spec 5): which parts of a value are digested, and how
 * each is read. It applies to any value: an object decoded from JSON, a
 * class instance, a `Map`.
 *
 * An identity is built from a kind (`new Identity(q.list(q.string()))`), or
 * read from its JSON form, which every implementation of quiddity reads and
 * writes, with {@link Identity.fromJson}. The type parameters are the type of
 * the values it describes and of the values it accepts, which builders
 * infer; an identity read from JSON accepts `unknown`.
 *
 * An identity is immutable and always valid (spec 5.9): every method that
 * changes one returns a new identity, and throws {@link InvalidIdentity}
 * when the result would not be valid. Its own digest is
 * `quiddity.digest(identity)` (spec 5.10).
 *
 * ```ts
 * import * as q from "quiddity";
 *
 * const book = q.Identity.fromJson('{"root": {"object": {"fields": {"title": {"kind": "string"}}}}}');
 * book.digest({ title: "Middlemarch", shelf: "B4" }).equals(book.digest({ title: "Middlemarch" })); // true
 * ```
 */
export class Identity<Natural = unknown, Accepted = unknown> {
  declare readonly __natural?: Natural;
  declare readonly __accepts?: (value: Accepted) => void;

  readonly #root: Kind;
  readonly #defs: Defs;
  readonly #domain: string | undefined;
  readonly #key: string;
  readonly #applier: Applier;

  /**
   * The identity of the values of `kind`, a kind built with quiddity's
   * builders, in the domain `options.domain` if given.
   */
  constructor(kind: Buildable, options?: { domain?: string | undefined });
  /** @internal */
  constructor(kind: typeof MAKE, parts: { root: Kind; defs: Defs; domain: string | undefined });
  constructor(kind: Buildable | typeof MAKE, options?: { domain?: string | undefined; root?: Kind; defs?: Defs }) {
    let root: Kind, defs: Defs, domain: string | undefined;
    if (kind === MAKE) {
      ({ root, defs, domain } = options as { root: Kind; defs: Defs; domain: string | undefined });
    } else {
      if (typeof kind !== "object" || kind === null || !(BUILD in kind)) {
        throw new TypeError("an Identity is built from a kind, such as q.object({...}); use Identity.fromJson for its JSON form");
      }
      const ctx = new BuildContext();
      const r = canonical(kind[BUILD](ctx), "$.root");
      ({ root, defs, domain } = build(r, ctx.defs, options?.domain));
    }
    this.#root = root;
    this.#defs = defs;
    this.#domain = domain;
    this.#key = canonKey(this.toJson());
    this.#applier = new Applier(new Checker(defs));
    Object.freeze(this);
  }

  /**
   * Reads an identity from its JSON form (spec 5.3): JSON text, or an object
   * decoded from it. Text is read strictly: a repeated member name is an
   * error, and so is a number where an integer is required, such as `5.0`
   * (which `JSON.parse` cannot tell from `5`).
   */
  static fromJson(source: string | IdentityJson | object): Identity {
    let doc: unknown;
    if (typeof source === "string") {
      try {
        doc = parseRawJson(source);
      } catch (e) {
        throw new InvalidIdentity(`not JSON: ${(e as Error).message}`);
      }
    } else if (typeof source === "object" && source !== null) {
      doc = source;
    } else {
      throw new TypeError(`an identity's JSON form is text or an object, not ${typeof source}`);
    }
    const { root, defs, domain } = load(doc);
    return new Identity(MAKE, { root, defs, domain });
  }

  /**
   * The identity's JSON form, in canonical form (spec 5.3): plain objects,
   * arrays, strings and integers (a bigint beyond 2^53). Integers beyond
   * 2^53 are rare (a discriminant); {@link toJsonText} writes them.
   */
  toJson(): IdentityJson {
    const out: IdentityJson = { root: copyJson(this.#root) };
    if (this.#domain !== undefined) out.domain = this.#domain;
    if (Object.keys(this.#defs).length > 0) out.definitions = copyJson(this.#defs) as Record<string, KindJson>;
    return out;
  }

  /** The identity's JSON form as JSON text. */
  toJsonText(): string {
    return jsonText(this.toJson());
  }

  /** The domain of digests under this identity (spec 4.2), or undefined. */
  get domain(): string | undefined {
    return this.#domain;
  }

  /** The name of the identity's own definition, if its root is one. */
  get name(): string | undefined {
    const r = this.#root;
    return typeof r === "object" && typeof r["ref"] === "string" ? r["ref"] : undefined;
  }

  /** The digest of `value` under this identity (spec 5.1). */
  digest(value: Accepted): Digest {
    const d = Digest.ofEncoding(encode(this.read(value)));
    return this.#domain !== undefined ? d.inDomain(this.#domain) : d;
  }

  /** The encoding (spec 3) of `value` under this identity: the bytes its digest hashes. */
  encoding(value: Accepted): Uint8Array {
    return encode(this.read(value));
  }

  /** The data-model value (spec 2) of `value` under this identity. */
  read(value: Accepted, path = "$"): Value {
    return this.#applier.read(this.#root, value, path);
  }

  /** Whether `other` is an identity with the same canonical form. */
  equals(other: unknown): boolean {
    return other instanceof Identity && other.#key === this.#key;
  }

  /**
   * The identity, named `name`: its root becomes its own definition under
   * that name, which is how it appears in its JSON form and in those of
   * identities it is nested in.
   */
  named(name: string): Identity<Natural, Accepted> {
    if (typeof name !== "string" || name === "") throw new InvalidIdentity("a name is a non-empty string");
    const old = this.name;
    if (old === name) return this;
    if (Object.hasOwn(this.#defs, name)) throw new InvalidIdentity(`the identity already has a definition named ${JSON.stringify(name)}`);
    const defs: Record<string, Kind> = {};
    for (const [n, k] of Object.entries(this.#defs)) {
      defs[n === old ? name : n] = old === undefined ? k : (renameRefs(k, old, name) as Kind);
    }
    if (old === undefined) defs[name] = this.#root;
    return Identity.#make({ ref: name }, defs, this.#domain);
  }

  /** The identity with another domain, or none. */
  withDomain(domain: string | undefined): Identity<Natural, Accepted> {
    return Identity.#make(this.#root, this.#defs, domain);
  }

  /**
   * The identity with the field `name` added to its root object, or
   * replaced; with `options.definition`, to that definition's object.
   * `field` is a kind or a field built with quiddity's builders:
   * `q.string().rename("label")`.
   */
  withField(name: string, field: BuildableField, options?: { definition?: string }): Identity {
    const ctx = new BuildContext(this.#defs);
    const f = field[BUILD_FIELD](ctx);
    return this.#change(options?.definition, ctx.defs, (o) => {
      o.fields[name] = f as unknown as ObjectKind["fields"][string];
      if (o.order !== undefined && !o.order.includes(name)) o.order.push(name);
    });
  }

  /** The identity without the field `name` of its root object, or of `options.definition`'s. */
  withoutField(name: string, options?: { definition?: string }): Identity {
    return this.#change(options?.definition, this.#defs, (o) => {
      if (!Object.hasOwn(o.fields, name)) throw new InvalidIdentity(`there is no field named ${JSON.stringify(name)}`);
      delete o.fields[name];
      if (o.order !== undefined) o.order = o.order.filter((n) => n !== name);
    });
  }

  #change(definition: string | undefined, defs: Defs, change: (o: ObjectKind) => void): Identity {
    let root = copyJson(this.#root);
    const newDefs = copyJson(defs) as Record<string, Kind>;
    const target = definition ?? this.name;
    let holder: Kind;
    if (target === undefined) {
      holder = root;
    } else {
      if (!Object.hasOwn(newDefs, target)) {
        throw new InvalidIdentity(`the identity has no definition named ${JSON.stringify(target)}`);
      }
      holder = newDefs[target]!;
    }
    if (!(typeof holder === "object" && Object.hasOwn(holder, "object"))) {
      const where = definition !== undefined ? `the definition ${JSON.stringify(definition)}` : "the identity's root";
      throw new InvalidIdentity(`${where} is not an object`);
    }
    change(holder["object"] as ObjectKind);
    root = canonical(root, "$.root");
    for (const [n, k] of Object.entries(newDefs)) newDefs[n] = canonical(k, `$.definitions.${n}`);
    return Identity.#make(root, newDefs, this.#domain);
  }

  static #make(root: Kind, defs: Record<string, Kind> | Defs, domain: string | undefined): Identity<any, any> {
    const parts = build(root, { ...defs }, domain);
    return new Identity(MAKE, parts);
  }

  /** As a field of an object built in code, the identity's root, with its definitions. */
  [BUILD](ctx: BuildContext): Kind {
    for (const [n, k] of Object.entries(this.#defs)) ctx.define(n, () => k);
    return this.#root;
  }

  [BUILD_FIELD](ctx: BuildContext): Record<string, unknown> {
    return { kind: this[BUILD](ctx) };
  }

  [READER](): ReadsInstances {
    return {
      readInstance: (x, path) => this.read(x as Accepted, path),
      instanceDomain: this.#domain,
    };
  }

  /** An identity is digested as its JSON form under the identity of identities (spec 5.10). */
  static [identitySymbol] = {
    [READER](): ReadsInstances {
      return {
        readInstance: (x: object, path: string) => meta().read((x as Identity).toJson(), path),
        instanceDomain: "quiddity.identity",
      };
    },
  };

  toString(): string {
    return `Identity(${this.toJsonText()})`;
  }
}

function renameRefs(k: unknown, old: string, name: string): unknown {
  if (Array.isArray(k)) return k.map((x) => renameRefs(x, old, name));
  if (typeof k === "object" && k !== null) {
    const keys = Object.keys(k);
    if (keys.length === 1 && keys[0] === "ref" && (k as { ref: unknown }).ref === old) return { ref: name };
    return Object.fromEntries(Object.entries(k).map(([key, v]) => [key, renameRefs(v, old, name)]));
  }
  return k;
}

let META: Identity | undefined;

/** The identity of identities (spec Appendix C). */
export function meta(): Identity {
  META ??= Identity.fromJson(IDENTITY_OF_IDENTITIES);
  return META;
}

/** An `Identity`, or a kind built in code as an identity without a domain. */
export function asIdentity(x: unknown): Identity {
  if (x instanceof Identity) return x;
  if (typeof x === "object" && x !== null && BUILD in x) {
    const cached = (x as { identity?: unknown }).identity;
    return cached instanceof Identity ? cached : new Identity(x as Buildable);
  }
  throw new QuiddityError("expected an Identity or a kind built with quiddity's builders");
}
