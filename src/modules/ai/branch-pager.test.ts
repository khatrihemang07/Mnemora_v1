import { describe, expect, it } from 'vitest';
import { pagerLabel, pagerStep } from './branch-pager';

describe('pagerLabel', () => {
  it('says which question of how many is shown', () => {
    expect(pagerLabel(2, 3)).toBe('2 of 3');
  });
});

describe('pagerStep', () => {
  it('moves to the question before or after this one', () => {
    expect(pagerStep(2, 3, 'previous')).toBe(1);
    expect(pagerStep(2, 3, 'next')).toBe(3);
  });

  it('has no step past either end', () => {
    expect(pagerStep(1, 3, 'previous')).toBeNull();
    expect(pagerStep(3, 3, 'next')).toBeNull();
  });

  it('has no step when the question has no siblings', () => {
    expect(pagerStep(1, 1, 'next')).toBeNull();
    expect(pagerStep(1, 1, 'previous')).toBeNull();
  });
});
