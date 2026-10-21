import { describe, expect, it } from 'vitest';
import type { RepeatRule } from '../ipc/bindings';
import fixtures from '../ipc/fixtures/recurrence-rules.json';
import {
  afterAllowed,
  anchoredOn,
  datesBetween,
  firstMatch,
  maskDays,
  nextDue,
  ordinal,
  preview,
  presetKey,
  repeatLabel,
  repeatPresets,
  ruleProblem,
  settleRule,
  stepDate,
  WEEKDAYS_MASK,
} from './when-repeat';

interface FixtureRule {
  freq: string;
  every: number;
  mode: string;
  weekdays?: number[];
  anchor?: number;
}
interface Case {
  name: string;
  op: 'first_match' | 'next_due' | 'preview' | 'invalid';
  rule: FixtureRule;
  today?: string;
  due?: string;
  from?: string;
  expect?: string | string[];
}

/** A fixture rule as the wire shape: weekdays become a bitmask, `anchor` the day of month. */
function wire(r: FixtureRule): RepeatRule {
  const mask = (r.weekdays ?? []).reduce((m, d) => m | (1 << d), 0);
  const days = r.freq === 'week' ? mask : (r.anchor ?? 0);
  return { freq: r.freq, every: r.every, days, mode: r.mode };
}

describe('the shared recurrence fixtures (the same file runs against the Rust engine)', () => {
  for (const c of fixtures.cases as Case[]) {
    it(c.name, () => {
      const rule = wire(c.rule);
      if (c.op === 'invalid') {
        expect(ruleProblem(rule)).not.toBeNull();
        return;
      }
      expect(ruleProblem(rule)).toBeNull();
      const settled = settleRule(rule);
      if (c.op === 'first_match') expect(firstMatch(settled, c.today as string)).toBe(c.expect);
      if (c.op === 'next_due') {
        expect(nextDue(settled, c.due as string, c.today as string)).toBe(c.expect);
      }
      if (c.op === 'preview') expect(preview(settled, c.from as string, 3)).toEqual(c.expect);
    });
  }
});

const rule = (
  freq: RepeatRule['freq'],
  every: number,
  days: number | null = null,
  mode = 'fixed',
): RepeatRule => ({ freq, every, days, mode });

describe('rules', () => {
  it('settling keeps days only where they mean something', () => {
    expect(settleRule(rule('day', 2, 5))).toEqual(rule('day', 2));
    expect(settleRule(rule('month', 1, 31, 'after'))).toEqual(rule('month', 1, null, 'after'));
    expect(settleRule(rule('week', 1, 18))).toEqual(rule('week', 1, 18));
    expect(settleRule(rule('week', 1, 0))).toEqual(rule('week', 1));
  });

  it('a month or year rule is anchored on the day of its Due date', () => {
    expect(anchoredOn(rule('month', 1), '2026-01-31').days).toBe(31);
    expect(anchoredOn(rule('year', 1), '2024-02-29').days).toBe(29);
    expect(anchoredOn(rule('month', 1, null, 'after'), '2026-01-31').days).toBeNull();
    expect(anchoredOn(rule('week', 1, 18), '2026-01-31').days).toBe(18);
  });

  it('stops at the year 9999', () => {
    expect(stepDate(rule('year', 1), '9999-06-01')).toBeNull();
    expect(stepDate(rule('day', 1), '9999-12-31')).toBeNull();
    expect(nextDue(rule('year', 1), '9999-06-01', '2026-10-21')).toBeNull();
  });

  it('after done is offered only with an interval', () => {
    expect(afterAllowed(rule('week', 1, 18))).toBe(false);
    expect(afterAllowed(rule('week', 2))).toBe(true);
    expect(afterAllowed(rule('day', 3))).toBe(true);
    expect(afterAllowed(rule('month', 1, 31))).toBe(true);
  });

  it('reads weekday masks Monday first', () => {
    expect(maskDays(0b001_0010)).toEqual([1, 4]);
    expect(maskDays(1 | 2)).toEqual([1, 0]);
    expect(maskDays(WEEKDAYS_MASK)).toEqual([1, 2, 3, 4, 5]);
  });
});

describe('rings on the calendar', () => {
  it('lists the dates the rule lands on inside the window', () => {
    const r = rule('week', 1, 0b001_0010);
    expect(datesBetween(r, '2026-10-22', '2026-10-19', '2026-11-08')).toEqual([
      '2026-10-26',
      '2026-10-29',
      '2026-11-02',
      '2026-11-05',
    ]);
    // A window that starts later leaves the earlier ones out.
    expect(datesBetween(r, '2026-10-22', '2026-11-01', '2026-11-08')).toEqual([
      '2026-11-02',
      '2026-11-05',
    ]);
  });
});

describe('labels', () => {
  it('names a rule briefly', () => {
    expect(repeatLabel(rule('day', 1))).toBe('Every day');
    expect(repeatLabel(rule('day', 3, null, 'after'))).toBe('Every 3 days after done');
    expect(repeatLabel(rule('week', 1))).toBe('Every week');
    expect(repeatLabel(rule('week', 2))).toBe('Every 2 weeks');
    expect(repeatLabel(rule('week', 1, WEEKDAYS_MASK))).toBe('Every weekday');
    expect(repeatLabel(rule('week', 1, 0b001_0010))).toBe('Every Mon, Thu');
    expect(repeatLabel(rule('week', 2, 0b001_0010))).toBe('Every 2 weeks on Mon, Thu');
    expect(repeatLabel(rule('week', 1, 1 | 2))).toBe('Every Mon, Sun');
    expect(repeatLabel(rule('month', 1, 31))).toBe('Every month');
    expect(repeatLabel(rule('year', 4))).toBe('Every 4 years');
  });

  it('writes ordinals', () => {
    expect([1, 2, 3, 4, 11, 12, 13, 21, 22, 23, 31].map(ordinal)).toEqual([
      '1st',
      '2nd',
      '3rd',
      '4th',
      '11th',
      '12th',
      '13th',
      '21st',
      '22nd',
      '23rd',
      '31st',
    ]);
  });
});

describe('presets', () => {
  it('are worded for the date', () => {
    const labels = repeatPresets('2026-10-21').map((p) => p.label);
    expect(labels).toEqual([
      'Every day',
      'Every 2 days',
      'Every 3 days',
      'Every 4 days',
      'Every weekday',
      'Weekly on Wednesday',
      'Monthly on the 21st',
      'Yearly on 21 Oct',
    ]);
  });

  it('are recognised from a rule, whatever its mode', () => {
    for (const p of repeatPresets('2026-10-21')) expect(presetKey(p.rule)).toBe(p.key);
    expect(presetKey(rule('day', 3, null, 'after'))).toBe('d3');
    expect(presetKey(rule('day', 5))).toBeNull();
    expect(presetKey(rule('week', 1, 0b001_0010))).toBeNull();
    expect(presetKey(rule('week', 2))).toBeNull();
    expect(presetKey(rule('month', 2))).toBeNull();
    expect(presetKey(rule('month', 1, 31))).toBe('month');
    expect(presetKey(null)).toBeNull();
  });
});
