import type { ConflictRow, ConflictSubject } from '../../lib/ipc/bindings';

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
export function rowTitle(subject: ConflictSubject): string {
  switch (subject.kind) {
    case 'entry':
      return `${pageLabel(subject.page_date)} · ${subject.written_time} Entry`;
    case 'task':
      return `‘${subject.title}’ · ${subject.field}`;
    case 'comment':
      return `‘${subject.task_title}’ · comment`;
  }
}

/** "09:40" in the viewer's clock. */
export function clockText(ms: number): string {
  const d = new Date(ms);
  return `${String(d.getHours()).padStart(2, '0')}:${String(d.getMinutes()).padStart(2, '0')}`;
}

/** "Mac 09:40 · Pixel 8 09:31": the showing version first. */
export function sidesText(row: Pick<ConflictRow, 'showing' | 'other'>): string {
  return `${row.showing.device} ${clockText(row.showing.at)} · ${row.other.device} ${clockText(row.other.at)}`;
}

export interface DayGroup {
  key: string;
  title: string;
  subtitle: string;
  rows: ConflictRow[];
}

/** Rows grouped by the day the newest version was written, newest day first (F11: by day). */
export function groupByDay(rows: ConflictRow[], now: Date = new Date()): DayGroup[] {
  const groups = new Map<string, DayGroup>();
  const dayKey = (d: Date) => `${d.getFullYear()}-${d.getMonth()}-${d.getDate()}`;
  const today = dayKey(now);
  const yesterday = dayKey(new Date(now.getFullYear(), now.getMonth(), now.getDate() - 1));
  for (const row of [...rows].sort((a, b) => b.raised_at - a.raised_at)) {
    const d = new Date(row.raised_at);
    const key = dayKey(d);
    let group = groups.get(key);
    if (!group) {
      const long = `${['Sunday', 'Monday', 'Tuesday', 'Wednesday', 'Thursday', 'Friday', 'Saturday'][d.getDay()]} ${d.getDate()} ${MONTHS[d.getMonth()]}`;
      group = {
        key,
        title: key === today ? 'Today' : key === yesterday ? 'Yesterday' : (DAYS[d.getDay()] ?? ''),
        subtitle: long,
        rows: [],
      };
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
