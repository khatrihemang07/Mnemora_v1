import type { ConflictRow, ConflictSubject } from '../../lib/ipc/bindings';
import { doneHeading } from '../../lib/time/day-heading';
import { clockParts, type TimeFormat } from '../../lib/time/format';

const DAYS = ['Sun', 'Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat'];
const MONTHS = ['Jan', 'Feb', 'Mar', 'Apr', 'May', 'Jun', 'Jul', 'Aug', 'Sep', 'Oct', 'Nov', 'Dec'];

/** "Wed 21 Oct" for a `YYYY-MM-DD` Page. */
export function pageLabel(pageDate: string): string {
  const [y, m, d] = pageDate.split('-').map(Number);
  if (!y || !m || !d) return pageDate;
  const date = new Date(Date.UTC(y, m - 1, d));
  return `${DAYS[date.getUTCDay()]} ${d} ${MONTHS[m - 1]}`;
}

/** What clashed and where (F2): "Wed 21 Oct · 09:12 Entry", "‘Plan weekend hike’ · notes". */
export function rowTitle(subject: ConflictSubject, format: TimeFormat): string {
  switch (subject.kind) {
    case 'entry':
      return `${pageLabel(subject.page_date)} · ${entryTime(subject.written_at, subject.written_offset_min, format)} Entry`;
    case 'task':
      return `‘${subject.title}’ · ${subject.field}`;
    case 'comment':
      return `‘${subject.task_title}’ · comment`;
    case 'conversation':
      return `‘${subject.title}’ · conversation title`;
    case 'saved_question':
      return `‘${subject.text}’ · saved question`;
  }
}

/** When an Entry was written, as the time of day in the zone it was written in. */
function entryTime(writtenAt: number, offsetMin: number, format: TimeFormat): string {
  const shifted = new Date(writtenAt + offsetMin * 60_000);
  // The same hour and minute on the viewer's clock, so `timeText` can format it.
  return timeText(
    new Date(1970, 0, 1, shifted.getUTCHours(), shifted.getUTCMinutes()).getTime(),
    format,
  );
}

/** "Mac 09:40 · Pixel 8 09:31": the showing version first. */
export function sidesText(row: Pick<ConflictRow, 'showing' | 'other'>, format: TimeFormat): string {
  return `${row.showing.device} ${timeText(row.showing.at, format)} · ${row.other.device} ${timeText(row.other.at, format)}`;
}

export interface DayGroup<T = ConflictRow> {
  key: string;
  title: string;
  subtitle: string;
  rows: T[];
}

/**
 * Rows grouped by the day `when` gives for each, newest day first (F11: by day). Conflicts group
 * by when the newest version was written, Resolved automatically by when it was decided.
 */
export function groupByDay<T = ConflictRow>(
  rows: T[],
  now: Date = new Date(),
  when: (row: T) => number = (row) => (row as ConflictRow).raised_at,
): DayGroup<T>[] {
  const groups = new Map<string, DayGroup<T>>();
  const pad = (n: number) => String(n).padStart(2, '0');
  const dayOf = (d: Date) => `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`;
  const today = dayOf(now);
  for (const row of [...rows].sort((a, b) => when(b) - when(a))) {
    const key = dayOf(new Date(when(row)));
    let group = groups.get(key);
    if (!group) {
      // The heading Done and Activity use, so the three screens read alike.
      const heading = doneHeading(key, today);
      group = { key, title: heading.name, subtitle: heading.small, rows: [] };
      groups.set(key, group);
    }
    group.rows.push(row);
  }
  return [...groups.values()];
}

/** The id in `/sync/conflicts/‹id›`, or null on any other address. */
export function conflictIdOf(pathname: string): string | null {
  const m = /^\/sync\/conflicts\/([^/]+)$/.exec(pathname);
  return m?.[1] ? decodeURIComponent(m[1]) : null;
}

/** "Mac" -> "Use Mac's". */
export function usePossessive(device: string): string {
  return `Use ${device}’s`;
}

/** "09:31" or "9:31 AM", the way the Time format Preference says. */
export function timeText(ms: number, format: TimeFormat): string {
  const { hm, period } = clockParts(new Date(ms), format);
  return period ? `${hm} ${period}` : hm;
}
