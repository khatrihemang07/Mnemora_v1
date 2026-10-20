import type { PageDirection } from './bindings';

/**
 * Which Page dates `journal_pages` returns, mirroring the Rust command: `older` is strictly before
 * the cursor, `from` is the cursor and older, `newer` is the nearest dates above it. Newest first.
 * `null` means the cursor is missing where one is needed.
 */
export function pageDates(
  dates: string[],
  cursor: string | null,
  direction: PageDirection,
  limit: number,
): string[] | null {
  const sorted = [...new Set(dates)].sort().reverse();
  if (direction === 'newer') {
    if (cursor === null) return null;
    return sorted.filter((d) => d > cursor).slice(-limit);
  }
  const keep = (d: string) => cursor === null || (direction === 'from' ? d <= cursor : d < cursor);
  return sorted.filter(keep).slice(0, limit);
}

/** A real calendar date written `YYYY-MM-DD` (mirrors `validate_page_date` in crates/journal). */
export function validDate(date: string): boolean {
  if (!/^\d{4}-\d{2}-\d{2}$/.test(date)) return false;
  const [y = 0, m = 1, d = 1] = date.split('-').map(Number);
  const probe = new Date(Date.UTC(y, m - 1, d));
  return (
    y >= 1 &&
    probe.getUTCFullYear() === y &&
    probe.getUTCMonth() === m - 1 &&
    probe.getUTCDate() === d
  );
}
