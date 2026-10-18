/** How kinds built in code become an identity's JSON form: definitions collected by name. */

import { InvalidIdentity } from "./errors.js";
import { type Kind, canonKey, canonical } from "./load.js";

/** The method by which a kind built in code writes its JSON form. */
export const BUILD: unique symbol = Symbol("quiddity.build");

/** The method by which a field built in code writes its JSON form. */
export const BUILD_FIELD: unique symbol = Symbol("quiddity.buildField");

/** A kind built in code. */
export interface Buildable {
  [BUILD](ctx: BuildContext): Kind;
}

/** A field built in code: a kind, or a kind with field options. */
export interface BuildableField {
  [BUILD_FIELD](ctx: BuildContext): Record<string, unknown>;
}

/** Collects the definitions of the named kinds an identity reaches. */
export class BuildContext {
  readonly defs: Record<string, Kind>;
  readonly #building = new Set<string>();

  constructor(defs: Record<string, Kind> = {}) {
    this.defs = { ...defs };
  }

  /** The kind `k` as a `ref` to the definition `name`, which is added unless an equal one is there. */
  define(name: string, build: () => Kind): Kind {
    if (this.#building.has(name)) return { ref: name };
    this.#building.add(name);
    let k: Kind;
    try {
      k = canonical(build(), `$.definitions.${name}`);
    } finally {
      this.#building.delete(name);
    }
    const found = this.defs[name];
    if (found !== undefined && canonKey(found) !== canonKey(k)) {
      throw new InvalidIdentity(`two different kinds are named ${JSON.stringify(name)}`, `$.definitions.${name}`);
    }
    this.defs[name] = k;
    return { ref: name };
  }
}
