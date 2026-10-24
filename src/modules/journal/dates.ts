import type { TimeFormat } from '../../lib/time/format';
import { addDays } from '../../lib/ui/calendar';

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

/** A calendar date has no zone: UTC on both ends keeps the viewer's zone from shifting it. */
function dateParts(date: string) {
  const [y = 0, mo = 1, d = 1] = date.split('-').map(Number);
  const at = new Date(Date.UTC(y, mo - 1, d));
  const part = (opts: Intl.DateTimeFormatOptions) =>
    new Intl.DateTimeFormat('en-GB', { ...opts, timeZone: 'UTC' }).format(at);
  return { y, d, at, part };
}

export interface PageHeading {
  /** "Tuesday, 20 October": the real date, always. */
  title: string;
  /** Only for a Page outside `today`'s year. */
  year: string | null;
  badge: 'Today' | 'Yesterday' | null;
}

/** The pieces of a Page header: date, year (other years only) and the Today/Yesterday badge. */
export function pageHeading(date: string, today: string): PageHeading {
  const { y, d, part } = dateParts(date);
  const badge = date === today ? 'Today' : date === addDays(today, -1) ? 'Yesterday' : null;
  return {
    title: `${part({ weekday: 'long' })}, ${d} ${part({ month: 'long' })}`,
    year: y === Number(today.slice(0, 4)) ? null : String(y),
    badge,
  };
}

/** "Mon 19 Oct": the short date stacked above an Entry's time. */
function shortDate(date: string): string {
  const { d, part } = dateParts(date);
  return `${part({ weekday: 'short' })} ${d} ${part({ month: 'short' })}`;
}

/** "Thu 15 Oct", with the year when it is not `today`'s year: a date named in a sentence or chip. */
export function dayLabel(date: string, today: string): string {
  const year = date.slice(0, 4);
  return shortDate(date) + (year !== today.slice(0, 4) ? ` ${year}` : '');
}

/** The colour token for a day's badge or chip: Today and Yesterday each have their own, any other day the Journal hue. */
export function dayHue(date: string, today: string): string {
  if (date === today) return '--day-today';
  if (date === addDays(today, -1)) return '--day-yesterday';
  return '--hue-journal';
}

/** The composer and editor date chip: "Today", "Yesterday" or the short date. */
export function chipLabel(date: string, today: string): string {
  if (date === today) return 'Today';
  if (date === addDays(today, -1)) return 'Yesterday';
  return dayLabel(date, today);
}

/** The calendar day on the wall clock of the zone an instant was written in. */
function wallDate(at: number, offsetMin: number): string {
  return new Date(at + offsetMin * MINUTE_MS).toISOString().slice(0, 10);
}

/**
 * The date to show with an Entry's time: null when it was written on the Page's own date (in the
 * zone it was written in), else a short date, with the year when it differs from the Page's.
 */
export function writtenDateLabel(
  writtenAt: number,
  offsetMin: number,
  pageDate: string,
): string | null {
  const wd = wallDate(writtenAt, offsetMin);
  if (wd === pageDate) return null;
  const year = wd.slice(0, 4);
  return shortDate(wd) + (year !== pageDate.slice(0, 4) ? ` ${year}` : '');
}

/** The tooltip text for an Entry's time: the full date, time and zone it was written in. */
export function fullWritten(
  writtenAt: number,
  offsetMin: number,
  tz: string,
  format: TimeFormat,
): string {
  const { y, d, part } = dateParts(wallDate(writtenAt, offsetMin));
  const date = `${part({ weekday: 'long' })}, ${d} ${part({ month: 'long' })} ${y}`;
  return `Written ${date} at ${writtenTime(writtenAt, offsetMin, format)} (${tz})`;
}

/** The tooltip text for the "edited" mark: "Edited Tue 20 Oct, 14:02". */
export function editedLabel(editedAt: number, offsetMin: number, format: TimeFormat): string {
  return `Edited ${shortDate(wallDate(editedAt, offsetMin))}, ${writtenTime(editedAt, offsetMin, format)}`;
}

/** The faint desktop header summary, e.g. "2 entries · 40 words". */
export function pageSummary(entries: number, words: number): string {
  return `${entries} ${entries === 1 ? 'entry' : 'entries'} · ${words} ${words === 1 ? 'word' : 'words'}`;
}

/** "September 2026": the month divider text (the UI uppercases it). */
export function monthLabel(date: string): string {
  const { y, part } = dateParts(date);
  return `${part({ month: 'long' })} ${y}`;
}

/**
 * The "↑ Today" pill shows once you are more than a screen below where the Journal opens (Today's
 * area: the top, or just under any future Pages), or at once while the loaded window is away from
 * Today altogether (`away`: a jump to a far date).
 */
export function showTodayPill(
  scrollTop: number,
  viewportHeight: number,
  todayTop = 0,
  away = false,
): boolean {
  return away || scrollTop - todayTop > viewportHeight;
}

/** Opening on Today lands within a few pixels of its top (row sizes are estimates until measured). */
const UPCOMING_SLACK_PX = 24;

/**
 * The "↑ Upcoming (n)" pill shows at the top of Today's area: from where Today starts down to a
 * screen below it (after which the "↑ Today" pill takes over), and only when future Pages exist.
 */
export function showUpcomingPill(
  scrollTop: number,
  viewportHeight: number,
  todayTop: number,
  upcoming: number,
  away = false,
): boolean {
  return (
    !away &&
    upcoming > 0 &&
    scrollTop >= todayTop - UPCOMING_SLACK_PX &&
    !showTodayPill(scrollTop, viewportHeight, todayTop)
  );
}
