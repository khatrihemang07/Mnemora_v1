import { describe, expect, it } from 'vitest';
import {
  compareFlow,
  discardEdit,
  draftIsCurrent,
  escapeCancels,
  forgetCompareFlow,
  stepBack,
} from './compare-flow.svelte';

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

describe('keeping the draft (R4)', () => {
  it('Back and Next keep the text while the choices are the same, and a changed choice makes it again', () => {
    const flow = compareFlow('keep');
    flow.choices = ['showing', 'other'];
    flow.draft = 'made';
    flow.draftFor = 'showing,other';
    flow.draft = 'made, then polished';
    expect(draftIsCurrent(flow)).toBe(true);
    flow.choices = ['showing', 'showing'];
    expect(draftIsCurrent(flow)).toBe(false);
    forgetCompareFlow('keep');
  });

  it('Cancel throws the half-made merge away', () => {
    const flow = compareFlow('cancel');
    flow.step = 'polish';
    flow.choices = ['other'];
    flow.draft = 'x';
    flow.draftFor = 'other';
    discardEdit(flow);
    expect(flow.step).toBe('compare');
    expect(flow.choices).toEqual([]);
    expect(draftIsCurrent(flow)).toBe(false);
    forgetCompareFlow('cancel');
  });
});

describe('Esc in Edit merged (T4)', () => {
  const esc = { key: 'Escape', defaultPrevented: false };
  it('cancels from Choose and Polish like the Cancel button', () => {
    for (const step of ['choose', 'polish'] as const) {
      const flow = compareFlow('esc');
      flow.step = step;
      flow.choices = ['other'];
      flow.draft = 'x';
      expect(escapeCancels(flow, esc)).toBe(true);
      expect(flow.step).toBe('compare');
      expect(flow.draft).toBe('');
    }
    forgetCompareFlow('esc');
  });

  it('leaves Compare, other keys and an Esc something else took alone', () => {
    const flow = compareFlow('esc2');
    expect(escapeCancels(flow, esc)).toBe(false);
    flow.step = 'choose';
    expect(escapeCancels(flow, { key: 'a', defaultPrevented: false })).toBe(false);
    expect(escapeCancels(flow, { key: 'Escape', defaultPrevented: true })).toBe(false);
    expect(flow.step).toBe('choose');
    forgetCompareFlow('esc2');
  });

  it('leaves an Esc that closes a Dialog or Sheet over the page alone', () => {
    const flow = compareFlow('esc3');
    flow.step = 'choose';
    expect(escapeCancels(flow, esc, () => true)).toBe(false);
    expect(flow.step).toBe('choose');
    expect(escapeCancels(flow, esc, () => false)).toBe(true);
    forgetCompareFlow('esc3');
  });
});

describe('Esc inside the Polish editor (T4)', () => {
  const inEditor = { closest: (sel: string) => (sel === '.cm-editor' ? {} : null) } as never;
  const esc = { key: 'Escape', defaultPrevented: true, target: inEditor };

  it('cancels although CodeMirror took the key', () => {
    const flow = compareFlow('esc4');
    flow.step = 'polish';
    flow.draft = 'x';
    expect(escapeCancels(flow, esc, () => false)).toBe(true);
    expect(flow.step).toBe('compare');
    expect(flow.draft).toBe('');
    forgetCompareFlow('esc4');
  });

  it('leaves the Esc that closes the slash menu, or a Dialog, alone', () => {
    const flow = compareFlow('esc5');
    flow.step = 'polish';
    expect(escapeCancels(flow, esc, () => false, true)).toBe(false);
    expect(escapeCancels(flow, esc, () => true)).toBe(false);
    expect(escapeCancels(flow, { ...esc, target: null }, () => false)).toBe(false);
    expect(flow.step).toBe('polish');
    forgetCompareFlow('esc5');
  });
});
