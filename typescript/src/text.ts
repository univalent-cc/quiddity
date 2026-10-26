/**
 * The conversions of spec 5.5 that read text, and exact arithmetic on
 * decimal and binary numbers for them and for quantization (spec 5.6).
 * Nothing here passes through binary floating point unless the spec says so.
 */

/** Removes ASCII whitespace from both ends of a string source (spec 5.5). */
export function trimSpace(s: string): string {
  return s.replace(/^[ \t\n\r]+|[ \t\n\r]+$/g, "");
}

/** An exact rational number, `n / d` with `d > 0`. */
export interface Rational {
  readonly n: bigint;
  readonly d: bigint;
}

/** The largest decimal exponent read; beyond it, a conversion is an error (spec 5.5 allows a limit). */
const MAX_EXPONENT = 100_000;

const DECIMAL = /^([+-]?)(0|[1-9][0-9]*)(?:\.([0-9]+))?(?:[eE]([+-]?[0-9]+))?$/;

/** A decimal number as `digits × 10^exponent`, exactly. */
export interface Decimal {
  readonly digits: bigint;
  readonly exponent: number;
}

/**
 * Decimal text (spec 5.5): a number in the syntax of RFC 8259, optionally
 * preceded by `+`, exactly; undefined if `text` is not one. Throws a
 * `RangeError` for an exponent too large to compute with.
 */
export function decimal(text: string): Decimal | undefined {
  const m = DECIMAL.exec(text);
  if (!m) return undefined;
  const fraction = m[3] ?? "";
  const exp = m[4] === undefined ? 0 : Number(m[4]);
  if (!Number.isSafeInteger(exp) || Math.abs(exp) > MAX_EXPONENT) throw new RangeError(`the exponent of ${text} is too large`);
  const digits = BigInt(m[2]! + fraction) * (m[1] === "-" ? -1n : 1n);
  return { digits, exponent: exp - fraction.length };
}

export function pow10(n: number): bigint {
  return 10n ** BigInt(n);
}

/** `a / b` rounded toward negative infinity, for `b > 0`. */
export function floorDiv(a: bigint, b: bigint): bigint {
  const q = a / b;
  return a % b !== 0n && a < 0n ? q - 1n : q;
}

/** A decimal as a rational, in lowest terms when it is an integer. */
export function decimalRational(x: Decimal): Rational {
  if (x.exponent >= 0) return { n: x.digits * pow10(x.exponent), d: 1n };
  const d = pow10(-x.exponent);
  return x.digits % d === 0n ? { n: x.digits / d, d: 1n } : { n: x.digits, d };
}

/** The exact value of a finite binary64 number, as a rational whose denominator is a power of 2. */
export function floatRational(x: number): Rational {
  const view = new DataView(new ArrayBuffer(8));
  view.setFloat64(0, x);
  const hi = view.getUint32(0);
  const lo = view.getUint32(4);
  const sign = hi >>> 31 ? -1n : 1n;
  const biased = (hi >>> 20) & 0x7ff;
  let mantissa = (BigInt(hi & 0xfffff) << 32n) | BigInt(lo);
  let exp: number;
  if (biased === 0) {
    exp = -1074;
  } else {
    mantissa |= 1n << 52n;
    exp = biased - 1075;
  }
  return exp >= 0 ? { n: sign * (mantissa << BigInt(exp)), d: 1n } : { n: sign * mantissa, d: 1n << BigInt(-exp) };
}

// --- bool, number, uuid ------------------------------------------------------------

export const BOOLS: Readonly<Record<string, boolean>> = {
  true: true, t: true, yes: true, y: true, on: true, "1": true,
  false: false, f: false, no: false, n: false, off: false, "0": false,
};

/** The words for NaN and the infinities (spec 5.5), lower case, with U+2212 as `-`. */
export const NUMBER_WORDS: Readonly<Record<string, number>> = {
  "nan": NaN, "+nan": NaN, "-nan": NaN,
  "inf": Infinity, "+inf": Infinity, "infinity": Infinity, "+infinity": Infinity, "∞": Infinity, "+∞": Infinity,
  "-inf": -Infinity, "-infinity": -Infinity, "-∞": -Infinity,
};

const UUID = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/;
const UUID_SIMPLE = /^[0-9a-f]{32}$/;

/** A UUID's canonical form (spec 5.5), or undefined if `text` is not a UUID. */
export function uuid(text: string): string | undefined {
  let t = trimSpace(text).toLowerCase();
  if (t.startsWith("{") && t.endsWith("}")) {
    t = t.slice(1, -1);
  } else if (t.startsWith("urn:uuid:")) {
    t = t.slice(9);
    if (!UUID.test(t)) return undefined;
  }
  if (UUID_SIMPLE.test(t)) t = `${t.slice(0, 8)}-${t.slice(8, 12)}-${t.slice(12, 16)}-${t.slice(16, 20)}-${t.slice(20)}`;
  return UUID.test(t) ? t : undefined;
}

// --- bytes ---------------------------------------------------------------------------

/** Hexadecimal text as bytes, or undefined if it is not an even number of hex digits. */
export function hexBytes(text: string): Uint8Array | undefined {
  if (!/^(?:[0-9a-fA-F]{2})*$/.test(text)) return undefined;
  const out = new Uint8Array(text.length / 2);
  for (let i = 0; i < out.length; i++) out[i] = parseInt(text.slice(i * 2, i * 2 + 2), 16);
  return out;
}

const BASE64 = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/**
 * Base64 text (RFC 4648) as bytes (spec 5.5): the standard or the URL-safe
 * alphabet but not both, padding correct or absent. A string says what is
 * wrong.
 */
export function base64Bytes(text: string): Uint8Array | string {
  const body = text.replace(/=+$/, "");
  const padding = text.length - body.length;
  if (/[+/]/.test(body) && /[-_]/.test(body)) return "base64 mixes the standard and URL-safe alphabets";
  if (padding > 0 && (text.length % 4 !== 0 || padding > 2)) return "incorrect base64 padding";
  if (body.length % 4 === 1) return "not base64";
  const std = body.replace(/-/g, "+").replace(/_/g, "/");
  if (!/^[A-Za-z0-9+/]*$/.test(std)) return "not base64";
  const out = new Uint8Array(Math.floor((std.length * 3) / 4));
  let bits = 0;
  let acc = 0;
  let n = 0;
  for (const c of std) {
    acc = (acc << 6) | BASE64.indexOf(c);
    bits += 6;
    if (bits >= 8) {
      bits -= 8;
      out[n++] = (acc >> bits) & 0xff;
    }
  }
  return out.subarray(0, n);
}

// --- instants and durations ----------------------------------------------------------

const RFC3339 =
  /^(\d{4})-(\d{2})-(\d{2})[Tt ](\d{2}):(\d{2}):(\d{2})(?:\.(\d{1,9}))?(?:([Zz])|([+-])(\d{2})(?::?(\d{2}))?)?$/;

/** Days since 1970-01-01 in the proleptic Gregorian calendar. */
function daysFromCivil(y: number, m: number, d: number): number {
  y -= m <= 2 ? 1 : 0;
  const era = Math.floor(y / 400);
  const yoe = y - era * 400;
  const doy = Math.floor((153 * (m + (m > 2 ? -3 : 9)) + 2) / 5) + d - 1;
  const doe = yoe * 365 + Math.floor(yoe / 4) - Math.floor(yoe / 100) + doy;
  return era * 146097 + doe - 719468;
}

function daysInMonth(y: number, m: number): number {
  if (m === 2) return (y % 4 === 0 && y % 100 !== 0) || y % 400 === 0 ? 29 : 28;
  return [4, 6, 9, 11].includes(m) ? 30 : 31;
}

/** An RFC 3339 date-time (spec 5.5), UTC without an offset, as nanoseconds since the epoch; a string says what is wrong. */
export function rfc3339(text: string): bigint | string {
  const m = RFC3339.exec(text);
  if (!m) return `not an RFC 3339 date-time: ${JSON.stringify(text)}`;
  const [y, mo, d, h, mi, s] = m.slice(1, 7).map(Number) as [number, number, number, number, number, number];
  // A second of 60 is a leap second, counted as the next minute's first (spec 2.6).
  if (!(mo >= 1 && mo <= 12 && d >= 1 && d <= daysInMonth(y, mo) && h <= 23 && mi <= 59 && s <= 60)) {
    return `date or time out of range: ${JSON.stringify(text)}`;
  }
  let offset = 0;
  if (m[9] !== undefined) {
    const oh = Number(m[10]);
    const om = Number(m[11] ?? 0);
    if (oh > 23 || om > 59) return `offset out of range: ${JSON.stringify(text)}`;
    offset = (oh * 3600 + om * 60) * (m[9] === "-" ? -1 : 1);
  }
  const secs = daysFromCivil(y, mo, d) * 86400 + h * 3600 + mi * 60 + s - offset;
  return BigInt(secs) * 1_000_000_000n + BigInt((m[7] ?? "").padEnd(9, "0"));
}

const ISO_DURATION =
  /^([+-])?[Pp](?:(\d+)[Ww])?(?:(\d+)[Dd])?(?:[Tt](?:(\d+)[Hh])?(?:(\d+)[Mm])?(?:(\d+)(?:\.(\d{1,9}))?[Ss])?)?$/;

/** An ISO 8601 duration without years or months (spec 5.5) as nanoseconds; a string says what is wrong. */
export function isoDuration(text: string): bigint | string {
  const m = ISO_DURATION.exec(text);
  if (!m || m.slice(2, 7).every((g) => g === undefined) || /[Tt]$/.test(text)) {
    return `not an ISO 8601 duration without years or months: ${JSON.stringify(text)}`;
  }
  const n = (g: string | undefined) => BigInt(g ?? 0);
  const secs = (n(m[2]) * 7n + n(m[3])) * 86_400n + n(m[4]) * 3_600n + n(m[5]) * 60n + n(m[6]);
  const ns = secs * 1_000_000_000n + BigInt((m[7] ?? "").padEnd(9, "0"));
  return m[1] === "-" ? -ns : ns;
}

// --- quantization (spec 5.6) ---------------------------------------------------------

/**
 * A number quantized to a multiple of `10^-d`, rounded down, exactly: an
 * integer as itself, a float as the shortest decimal that rounds to it.
 */
export function quantizeDecimals(v: bigint | number, d: number): bigint | number {
  if (typeof v === "number" && !Number.isFinite(v)) return v;
  // String(v) is the shortest decimal that rounds to a binary64 value.
  const x: Decimal = typeof v === "bigint" ? { digits: v, exponent: 0 } : decimal(String(v))!;
  const scaled = x.exponent + d;
  const k = scaled >= 0 ? x.digits * pow10(scaled) : floorDiv(x.digits, pow10(-scaled));
  if (d <= 0) return k * pow10(-d);
  const step = pow10(d);
  if (k % step === 0n) return k / step;
  const r = Number(`${k}e-${d}`);
  return Number.isInteger(r) ? BigInt(r) : r;
}
