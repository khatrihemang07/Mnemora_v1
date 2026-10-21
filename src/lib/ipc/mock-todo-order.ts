/**
 * Fractional-index keys for the browser mock. Mirrors crates/todo/src/order.rs: a key is a string
 * of base-62 digits read as a fraction in (0, 1) that sorts bytewise, and there is always one
 * strictly between any two others, so a move rewrites one row and never renumbers the rest.
 */

import type { AppError } from './bindings';

/** Base-62 digits in byte order, so keys sort the same as text and as fractions. */
const DIGITS = '0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz';
const MID = 31;

const BAD: AppError = {
  code: 'validation',
  message: 'Those two places cannot be told apart',
  details: null,
};

const digit = (c: string | undefined): number | null => {
  if (c === undefined) return null;
  const at = DIGITS.indexOf(c);
  return at < 0 ? null : at;
};

/** The key without trailing zeros: how keys are compared. */
const canonical = (key: string): string => key.replace(/0+$/, '');
const isKey = (key: string): boolean => [...key].every((c) => DIGITS.includes(c));

/** Rocicorp's midpoint: `lo` is canonical or empty, `hi` canonical or null (the end), `lo < hi`. */
function midpoint(lo: string, hi: string | null): string {
  if (hi !== null) {
    let n = 0;
    while (n < hi.length && (lo[n] ?? '0') === hi[n]) n++;
    if (n > 0) return hi.slice(0, n) + midpoint(lo.slice(n), hi.slice(n));
  }
  const digitLo = digit(lo[0]) ?? 0;
  const digitHi = hi === null ? DIGITS.length : (digit(hi[0]) ?? 0);
  if (digitHi - digitLo > 1) return DIGITS[Math.ceil((digitLo + digitHi) / 2)] as string;
  if (hi !== null && hi.length > 1) return hi.slice(0, 1);
  return (DIGITS[digitLo] as string) + midpoint(lo.slice(1), null);
}

/**
 * Mirrors `key_between`: a key strictly between `lo` and `hi` (null is no bound). Throws a
 * validation error when a bound is not a key or `lo` is not below `hi`.
 */
export function keyBetween(lo: string | null, hi: string | null): string {
  const l = lo === null ? null : canonical(lo);
  const h = hi === null ? null : canonical(hi);
  if ((l !== null && !isKey(l)) || (h !== null && !isKey(h))) throw BAD;
  if (l !== null && h !== null && l >= h) throw BAD;
  if (h === '') throw BAD;
  return midpoint(l ?? '', h);
}

/** Mirrors `key_after`: the key that goes after every other, given the last. */
export function keyAfter(last: string | null): string {
  const l = last === null ? '' : canonical(last);
  if (!isKey(l)) return l + (DIGITS[MID] as string);
  const top = DIGITS.length - 1;
  const i = [...l].findIndex((c) => (digit(c) ?? top) < top);
  if (i < 0) return l + (DIGITS[MID] as string);
  return l.slice(0, i) + (DIGITS[(digit(l[i]) ?? 0) + 1] as string);
}

/**
 * Mirrors `place_key`: the key that puts something between its neighbours' keys, or null to leave
 * it be. Neighbours that are out of order, or not adjacent in the whole order, put the new key
 * just after `before` or just before `after`.
 */
export function placeKey(
  before: string | null,
  after: string | null,
  keyBefore: (key: string) => string | null,
  keyFollowing: (key: string) => string | null,
): string | null {
  if (before === null && after === null) return null;
  if (before !== null && after !== null && canonical(before) < canonical(after)) {
    return keyBetween(before, after);
  }
  if (before !== null) return keyBetween(before, keyFollowing(before));
  return keyBetween(keyBefore(after as string), after);
}
