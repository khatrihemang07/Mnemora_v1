import { describe, expect, it } from 'vitest';
import { compareFlow, forgetCompareFlow } from './compare-flow.svelte';

describe('the Edit merged flow', () => {
  it('is the same flow when the page is mounted again', () => {
    const first = compareFlow('c1');
    first.step = 'polish';
    first.choices = ['other', 'both'];
    first.draft = 'merged words';
    const again = compareFlow('c1');
    expect(again.step).toBe('polish');
    expect(again.choices).toEqual(['other', 'both']);
    expect(again.draft).toBe('merged words');
    forgetCompareFlow('c1');
  });

  it('keeps conflicts apart and starts over once one is settled', () => {
    compareFlow('a').step = 'choose';
    expect(compareFlow('b').step).toBe('compare');
    forgetCompareFlow('a');
    expect(compareFlow('a').step).toBe('compare');
    forgetCompareFlow('a');
    forgetCompareFlow('b');
  });
});
