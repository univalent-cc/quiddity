/** The errors quiddity throws. */

/**
 * A value that has no digest: it, or a part of it, has no data-model value,
 * or it does not have the shape an identity describes.
 *
 * `path` says where, as a JSONPath-like string: `$` for the value itself,
 * `$.tags[2]` for an item inside it. `detail` is the message without the
 * path.
 */
export class QuiddityError extends Error {
  readonly path: string;
  readonly detail: string;

  constructor(detail: string, path = "$") {
    super(`${detail} (at ${path})`);
    this.path = path;
    this.detail = detail;
  }

  override get name(): string {
    return "QuiddityError";
  }
}

/** An identity, read or being built, that is not valid (spec 5.9). */
export class InvalidIdentity extends QuiddityError {
  override get name(): string {
    return "InvalidIdentity";
  }
}
