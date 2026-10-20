import { describe, expect, it } from 'vitest';
import { columnWidth, readJournal, WIDTH_REM } from './settings';

describe('readJournal', () => {
  it('uses the defaults when nothing is saved', () => {
    expect(readJournal()).toEqual({ font: 'serif', width: 'default', pageSummary: true });
  });
  it('reads saved values', () => {
    expect(
      readJournal({
        'journal.font': 'sans',
        'journal.width': 'wide',
        'journal.page_summary': false,
      }),
    ).toEqual({ font: 'sans', width: 'wide', pageSummary: false });
  });
  it('falls back for values it does not know', () => {
    expect(readJournal({ 'journal.font': 'mono', 'journal.width': 7 }).font).toBe('serif');
    expect(readJournal({ 'journal.width': 7 }).width).toBe('default');
  });
});

describe('column width', () => {
  it('maps the three options to growing rem widths, Default staying the old 42.5rem', () => {
    expect(WIDTH_REM.narrow).toBeLessThan(WIDTH_REM.default);
    expect(WIDTH_REM.default).toBeLessThan(WIDTH_REM.wide);
    expect(columnWidth('default')).toBe('42.5rem');
    expect(columnWidth('narrow')).toBe('35.5rem');
    expect(columnWidth('wide')).toBe('53rem');
  });
});
