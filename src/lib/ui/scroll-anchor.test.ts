import { describe, expect, it } from 'vitest';
import { anchoredScrollTop } from './scroll-anchor';

const anchor = { key: 'b', index: 1, delta: 5 };

describe('anchoredScrollTop', () => {
  it('follows the row when rows were added above it', () => {
    expect(anchoredScrollTop(anchor, ['x', 'y', 'a', 'b'], (i) => i * 100)).toEqual({
      top: 305,
      index: 3,
    });
  });
  it('does nothing when the row did not move, is gone, or there is no anchor', () => {
    expect(anchoredScrollTop(anchor, ['a', 'b'], () => 0)).toBeNull();
    expect(anchoredScrollTop(anchor, ['a', 'c'], () => 0)).toBeNull();
    expect(anchoredScrollTop(null, ['a', 'b'], () => 0)).toBeNull();
  });
});
