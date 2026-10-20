import { describe, expect, it } from 'vitest';
import { pinnedRow } from './pinned-row';

// Rows: a sticky Page at 0 (300px tall), a month divider at 300, a sticky Page at 350.
const rows = [
  { index: 0, start: 0 },
  { index: 1, start: 300 },
  { index: 2, start: 350 },
];
const sticky = (i: number) => i !== 1;

describe('pinnedRow', () => {
  it('pins the first Page at the very top', () => {
    expect(pinnedRow(rows, 0, sticky, 40)).toEqual({ index: 0, offset: 0 });
  });
  it('pins the Page being read', () => {
    expect(pinnedRow(rows, 120, sticky, 40)).toEqual({ index: 0, offset: 0 });
  });
  it('lets the next Page header push it up', () => {
    expect(pinnedRow(rows, 330, sticky, 40)).toEqual({ index: 0, offset: -20 });
  });
  it('switches once the next header reaches the top', () => {
    expect(pinnedRow(rows, 350, sticky, 40)).toEqual({ index: 2, offset: 0 });
  });
  it('ignores rows that are not sticky and is null without any', () => {
    expect(pinnedRow([{ index: 1, start: 0 }], 10, sticky, 40)).toBeNull();
  });
});
