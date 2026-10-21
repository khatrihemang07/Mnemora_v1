/** Fractional keys in the mock: the same cases as the Rust tests in crates/todo/src/order.rs. */
import { describe, expect, it } from 'vitest';
import { keyAfter, keyBetween, placeKey } from './mock-todo-order';

function between(lo: string | null, hi: string | null): string {
  const key = keyBetween(lo, hi);
  expect(key.endsWith('0')).toBe(false);
  if (lo !== null) expect(key > lo.replace(/0+$/, '')).toBe(true);
  if (hi !== null) expect(key < hi.replace(/0+$/, '')).toBe(true);
  return key;
}

describe('fractional keys', () => {
  it('starts mid-range', () => {
    expect(between(null, null)).toBe('V');
  });

  it('fits a key between any two neighbours', () => {
    const keys = ['1', 'V', 'W', 'Vk', 'z', 'zz', '0V', '00001', 'a0b', 'Vz', 'VzzV'];
    for (const lo of keys) for (const hi of keys) if (lo < hi) between(lo, hi);
  });

  it('fits before the first and after the last', () => {
    let first = 'V';
    for (let i = 0; i < 60; i++) {
      const before = between(null, first);
      expect(before < first).toBe(true);
      first = before;
    }
    let last = 'V';
    for (let i = 0; i < 200; i++) {
      const after = keyAfter(last);
      expect(after > last).toBe(true);
      expect(after.endsWith('0')).toBe(false);
      last = after;
    }
    expect(between('zzz', null) > 'zzz').toBe(true);
  });

  it('keeps appends short', () => {
    let last: string | null = null;
    for (let i = 0; i < 500; i++) last = keyAfter(last);
    expect((last as string).length).toBeLessThanOrEqual(18);
  });

  it('survives many inserts in the same gap without renumbering', () => {
    const [lo, hi] = ['V', 'W'];
    let mid = lo;
    const after: string[] = [];
    for (let i = 0; i < 1000; i++) {
      mid = between(mid, hi);
      after.push(mid);
    }
    expect(after.every((k, i) => i === 0 || (after[i - 1] as string) < k)).toBe(true);
    mid = hi;
    const before: string[] = [];
    for (let i = 0; i < 1000; i++) {
      mid = between(lo, mid);
      before.push(mid);
    }
    expect(before.every((k, i) => i === 0 || (before[i - 1] as string) > k)).toBe(true);
    expect(mid.length).toBeLessThan(400);
  });

  it('keeps a shuffled list sorted when only the moved key is written', () => {
    let seed = 12345;
    const next = (n: number) => {
      seed = (Math.imul(seed, 1103515245) + 12345) & 0x7fffffff;
      return seed % n;
    };
    const keys: string[] = [];
    for (let i = 0; i < 40; i++) keys.push(keyAfter(keys[keys.length - 1] ?? null));
    for (let i = 0; i < 3000; i++) {
      keys.splice(next(keys.length), 1);
      const at = next(keys.length + 1);
      keys.splice(at, 0, between(keys[at - 1] ?? null, keys[at] ?? null));
    }
    expect(keys.every((k, i) => i === 0 || (keys[i - 1] as string) < k)).toBe(true);
  });

  it('reads the old keys of the first slices, with trailing zeros', () => {
    expect(keyAfter('V0000000')).toBe('W');
    const key = between('V0000000', 'V0000001');
    expect(key > 'V0000000' && key < 'V0000001').toBe(true);
    expect(() => keyBetween('V', 'V0000000')).toThrow();
    expect(() => keyBetween('W', 'V')).toThrow();
    expect(() => keyBetween('V', 'V')).toThrow();
    expect(() => keyBetween('a b', null)).toThrow();
    expect(() => keyBetween(null, '000')).toThrow();
    expect(keyAfter('a b') > 'a b').toBe(true);
  });

  it('places a key by its neighbours, even out of order', () => {
    const rows = ['1', '3', '5', '7'];
    const before = (k: string) => rows.filter((r) => r < k).pop() ?? null;
    const following = (k: string) => rows.find((r) => r > k) ?? null;
    expect(placeKey(null, null, before, following)).toBeNull();
    const mid = placeKey('1', '5', before, following) as string;
    expect(mid > '1' && mid < '5').toBe(true);
    // Out of order: just after `before`, before whatever follows it in the whole order.
    const inverted = placeKey('5', '1', before, following) as string;
    expect(inverted > '5' && inverted < '7').toBe(true);
    const onlyAfter = placeKey(null, '5', before, following) as string;
    expect(onlyAfter > '3' && onlyAfter < '5').toBe(true);
  });
});
