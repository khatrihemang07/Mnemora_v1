import { describe, expect, it } from 'vitest';
import type { Hit } from '../../lib/ipc/bindings';
import {
  canSearch,
  closeSearch,
  entriesLabel,
  escapeSearch,
  goBack,
  groupHits,
  IDLE,
  moveActive,
  openResult,
  openSearch,
  openSearchFor,
  presetRange,
  rangeLabel,
  snippetText,
  toggleSearch,
  wantsBack,
  withQuery,
  withRange,
} from './search-state';

const hit = (id: string, page_date: string): Hit => ({
  id,
  page_date,
  written_at: 0,
  written_offset_min: 0,
  written_tz: 'UTC',
  snippet: [{ text: id, matched: false }],
});

describe('presetRange', () => {
  it('counts back a week, a month and a year from today', () => {
    expect(presetRange('week', '2026-10-20')).toEqual({
      from: '2026-10-13',
      to: '2026-10-20',
      preset: 'week',
    });
    expect(presetRange('month', '2026-10-20').from).toBe('2026-09-20');
    expect(presetRange('year', '2026-10-20').from).toBe('2025-10-20');
  });

  it('clamps to the last day of a shorter month and crosses year ends', () => {
    expect(presetRange('month', '2026-03-31').from).toBe('2026-02-28');
    expect(presetRange('year', '2028-02-29').from).toBe('2027-02-28');
    expect(presetRange('week', '2026-01-03').from).toBe('2025-12-27');
    expect(presetRange('month', '2026-01-15').from).toBe('2025-12-15');
  });
});

describe('rangeLabel', () => {
  const today = '2026-10-20';
  it('says Any date, the preset name, or the custom dates', () => {
    expect(rangeLabel(null, today)).toBe('Any date');
    expect(rangeLabel(presetRange('month', today), today)).toBe('Past month');
    expect(rangeLabel({ from: '2026-10-05', to: '2026-10-12', preset: null }, today)).toBe(
      '5 Oct – 12 Oct',
    );
    expect(rangeLabel({ from: '2026-10-05', to: '2026-10-05', preset: null }, today)).toBe('5 Oct');
  });
  it('adds the year to dates outside this year', () => {
    expect(rangeLabel({ from: '2025-12-30', to: '2026-01-02', preset: null }, today)).toBe(
      '30 Dec 2025 – 2 Jan',
    );
  });
});

describe('canSearch', () => {
  it('needs words or a range', () => {
    expect(canSearch({ query: '  ', range: null })).toBe(false);
    expect(canSearch({ query: 'a', range: null })).toBe(true);
    expect(canSearch({ query: '', range: presetRange('week', '2026-10-20') })).toBe(true);
  });
});

describe('search session', () => {
  const range = presetRange('week', '2026-10-20');

  it('opens blank and closing clears the query and the filter', () => {
    let s = openSearch(IDLE);
    expect(s).toMatchObject({ active: true, filter: { query: '', range: null } });
    s = withRange(withQuery(s, 'river'), range);
    expect(closeSearch()).toEqual(IDLE);
    expect(openSearch(closeSearch()).filter).toEqual({ query: '', range: null });
  });

  it('opening a result parks the results and Back returns to the same query and range', () => {
    let s = withRange(withQuery(openSearch(IDLE), 'river'), range);
    s = openResult(s);
    expect(s.active).toBe(false);
    expect(wantsBack(s)).toBe(true);
    s = goBack(s);
    expect(s).toEqual({ active: true, filter: { query: 'river', range }, parked: null });
  });

  it('opening search again (⌘F) also resumes parked results', () => {
    const parked = openResult(withQuery(openSearch(IDLE), 'river'));
    expect(openSearch(parked).filter.query).toBe('river');
  });

  it('Back with nothing parked leaves search, clearing it', () => {
    const s = withQuery(openSearch(IDLE), 'river');
    expect(goBack(s)).toEqual(IDLE);
    expect(wantsBack(IDLE)).toBe(false);
    expect(wantsBack(s)).toBe(true);
  });

  it('after resuming and closing, nothing is left to return to', () => {
    const resumed = goBack(openResult(withQuery(openSearch(IDLE), 'river')));
    expect(closeSearch()).toEqual(IDLE);
    expect(goBack(resumed)).toEqual(IDLE);
  });

  it('Escape clears the words first, then closes', () => {
    const s = withRange(withQuery(openSearch(IDLE), 'river'), range);
    const cleared = escapeSearch(s);
    expect(cleared.active).toBe(true);
    expect(cleared.filter).toEqual({ query: '', range });
    expect(escapeSearch(cleared)).toEqual(IDLE);
  });

  it('a palette query opens a fresh search holding it', () => {
    expect(openSearchFor('river')).toEqual({
      active: true,
      filter: { query: 'river', range: null },
      parked: null,
    });
  });

  it('the header button opens a blank search from idle and closes an active one', () => {
    expect(toggleSearch(IDLE)).toEqual(openSearch(IDLE));
    expect(toggleSearch(withQuery(openSearch(IDLE), 'river'))).toEqual(IDLE);
  });

  it('the header button on parked results clears them for good', () => {
    const parked = openResult(withRange(withQuery(openSearch(IDLE), 'river'), range));
    const s = toggleSearch(parked);
    expect(s).toEqual(IDLE);
    expect(openSearch(s).filter).toEqual({ query: '', range: null });
  });

  it('opening a result while not searching changes nothing', () => {
    expect(openResult(IDLE)).toBe(IDLE);
  });
});

describe('groupHits', () => {
  it('groups consecutive hits by Page and labels other years', () => {
    const groups = groupHits(
      [hit('a', '2026-10-20'), hit('b', '2026-10-20'), hit('c', '2025-03-02')],
      '2026-10-20',
    );
    expect(groups.map((g) => [g.label, g.start, g.hits.map((h) => h.id)])).toEqual([
      ['Tue 20 Oct', 0, ['a', 'b']],
      ['Sun 2 Mar 2025', 2, ['c']],
    ]);
    expect(groupHits([hit('d', '2026-10-01')], '2026-10-20', 3)[0]?.start).toBe(3);
  });
  it('is empty for no hits', () => {
    expect(groupHits([], '2026-10-20')).toEqual([]);
  });
});

describe('small helpers', () => {
  it('counts entries', () => {
    expect(entriesLabel(1)).toBe('1 entry');
    expect(entriesLabel(0)).toBe('0 entries');
    expect(entriesLabel(5)).toBe('5 entries');
  });
  it('joins snippet spans without marks', () => {
    expect(
      snippetText([
        { text: 'a ', matched: false },
        { text: 'b', matched: true },
      ]),
    ).toBe('a b');
  });
  it('moves the highlight and stops at the ends', () => {
    expect(moveActive(-1, 'ArrowDown', 3)).toBe(0);
    expect(moveActive(2, 'ArrowDown', 3)).toBe(2);
    expect(moveActive(0, 'ArrowUp', 3)).toBe(-1);
    expect(moveActive(-1, 'ArrowUp', 3)).toBe(-1);
    expect(moveActive(0, 'ArrowDown', 0)).toBe(-1);
  });
});
