import { describe, expect, it } from 'vitest';
import { pageDates } from './mock-journal-pages';

const DATES = ['2025-03-01', '2025-03-02', '2025-03-02', '2025-06-10', '2026-10-20', '2026-12-25'];

describe('pageDates', () => {
  it('older is strictly before the cursor, newest first', () => {
    expect(pageDates(DATES, '2025-06-10', 'older', 10)).toEqual(['2025-03-02', '2025-03-01']);
    expect(pageDates(DATES, null, 'older', 2)).toEqual(['2026-12-25', '2026-10-20']);
  });

  it('from starts at the cursor date itself', () => {
    expect(pageDates(DATES, '2025-06-10', 'from', 2)).toEqual(['2025-06-10', '2025-03-02']);
    expect(pageDates(DATES, '2025-12-31', 'from', 1)).toEqual(['2025-06-10']);
  });

  it('newer takes the nearest dates above the cursor, still newest first', () => {
    expect(pageDates(DATES, '2025-03-02', 'newer', 2)).toEqual(['2026-10-20', '2025-06-10']);
    expect(pageDates(DATES, '2026-10-20', 'newer', 10)).toEqual(['2026-12-25']);
    expect(pageDates(DATES, '2026-12-25', 'newer', 10)).toEqual([]);
  });

  it('newer without a cursor is invalid', () => {
    expect(pageDates(DATES, null, 'newer', 2)).toBeNull();
  });
});
