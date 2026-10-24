import { describe, expect, it } from 'vitest';
import { compareFlow, forgetCompareFlow, stepBack } from './compare-flow.svelte';

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

describe('Back inside Edit merged (R4)', () => {
  it('steps back one step and keeps the choices and the draft', () => {
    const flow = compareFlow('back');
    flow.step = 'polish';
    flow.choices = ['other'];
    flow.draft = 'my polish';
    flow.step = stepBack(flow.step) ?? flow.step;
    expect(flow.step).toBe('choose');
    flow.step = stepBack(flow.step) ?? flow.step;
    expect(flow.step).toBe('compare');
    expect(flow.choices).toEqual(['other']);
    expect(flow.draft).toBe('my polish');
    forgetCompareFlow('back');
  });

  it('has nowhere to step back from Compare, so Back leaves the page', () => {
    expect(stepBack('compare')).toBeNull();
  });
});
