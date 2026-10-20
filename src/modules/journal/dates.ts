import type { TimeFormat } from '../../lib/time/format';

const MINUTE_MS = 60_000;
const pad = (n: number) => String(n).padStart(2, '0');

/** `YYYY-MM-DD` of `now` on the device's calendar: the default Page for a new Entry. */
export function localDateString(now: Date): string {
  return `${now.getFullYear()}-${pad(now.getMonth() + 1)}-${pad(now.getDate())}`;
}

/** The wall-clock time an Entry was written, in the zone it was written in (not the viewer's). */
export function writtenTime(writtenAt: number, offsetMin: number, format: TimeFormat): string {
  const wall = new Date(writtenAt + offsetMin * MINUTE_MS);
  const h = wall.getUTCHours();
  const m = pad(wall.getUTCMinutes());
  if (format === '24h') return `${pad(h)}:${m}`;
  return `${h % 12 || 12}:${m} ${h < 12 ? 'AM' : 'PM'}`;
}

/** Page header text, e.g. "Tuesday, 20 October"; the year is added for other years than `today`'s. */
export function pageTitle(date: string, today: string): string {
  const [y = 0, mo = 1, d = 1] = date.split('-').map(Number);
  // UTC on both ends: a calendar date has no zone, so the viewer's zone must not shift it.
  const at = new Date(Date.UTC(y, mo - 1, d));
  const part = (opts: Intl.DateTimeFormatOptions) =>
    new Intl.DateTimeFormat('en-GB', { ...opts, timeZone: 'UTC' }).format(at);
  const text = `${part({ weekday: 'long' })}, ${d} ${part({ month: 'long' })}`;
  return y === Number(today.slice(0, 4)) ? text : `${text} ${y}`;
}
