/** Date maths for Calendar and DatePicker. A calendar date is `YYYY-MM-DD` and has no zone. */

export type WeekStart = 'mon' | 'sun';

export interface GridDay {
  /** `YYYY-MM-DD`. */
  date: string;
  day: number;
  /** False for the leading and trailing days of the neighbouring months. */
  inMonth: boolean;
}

const DAY_MS = 86_400_000;
const pad = (n: number, width = 2) => String(n).padStart(width, '0');

export const formatYmd = (y: number, m: number, d: number) => `${pad(y, 4)}-${pad(m)}-${pad(d)}`;

/** `[year, month (1-12), day]` of a `YYYY-MM-DD` date or a `YYYY-MM` month. */
export function parseYmd(date: string): [number, number, number] {
  const [y = 1970, m = 1, d = 1] = date.split('-').map(Number);
  return [y, m, d];
}

const utc = (date: string) => {
  const [y, m, d] = parseYmd(date);
  return new Date(Date.UTC(y, m - 1, d));
};

/** The calendar date `days` after `date` (negative for before). */
export function addDays(date: string, days: number): string {
  return new Date(utc(date).getTime() + days * DAY_MS).toISOString().slice(0, 10);
}

/** `month` is `YYYY-MM`; the result is the month `delta` months on (negative for before). */
export function shiftMonth(month: string, delta: number): string {
  const [y, m] = parseYmd(month);
  const index = y * 12 + (m - 1) + delta;
  return `${pad(Math.floor(index / 12), 4)}-${pad((index % 12) + 1)}`;
}

/** Position of a weekday in a week: 0 is the first day of the week. */
const weekdayIndex = (date: string, weekStart: WeekStart) =>
  (utc(date).getUTCDay() - (weekStart === 'sun' ? 0 : 1) + 7) % 7;

/**
 * Six full weeks (42 days) covering `month` (`YYYY-MM`), starting on the week's first day. Always
 * six rows, so the Calendar never changes height between months.
 */
export function monthGrid(month: string, weekStart: WeekStart): GridDay[] {
  const first = `${month}-01`;
  const start = addDays(first, -weekdayIndex(first, weekStart));
  return Array.from({ length: 42 }, (_, i) => {
    const date = addDays(start, i);
    return { date, day: parseYmd(date)[2], inMonth: date.startsWith(month) };
  });
}

const WEEKDAYS = ['Mo', 'Tu', 'We', 'Th', 'Fr', 'Sa', 'Su'];

/** Two-letter weekday headings in grid order. */
export function weekdayLabels(weekStart: WeekStart): string[] {
  return weekStart === 'mon' ? WEEKDAYS : [WEEKDAYS[6] ?? 'Su', ...WEEKDAYS.slice(0, 6)];
}

/** Where a key moves the focused day, or null when the key does not navigate the grid. */
export function moveFocus(date: string, key: string, weekStart: WeekStart): string | null {
  switch (key) {
    case 'ArrowLeft':
      return addDays(date, -1);
    case 'ArrowRight':
      return addDays(date, 1);
    case 'ArrowUp':
      return addDays(date, -7);
    case 'ArrowDown':
      return addDays(date, 7);
    case 'Home':
      return addDays(date, -weekdayIndex(date, weekStart));
    case 'End':
      return addDays(date, 6 - weekdayIndex(date, weekStart));
    case 'PageUp':
      return shiftMonthKeepingDay(date, -1);
    case 'PageDown':
      return shiftMonthKeepingDay(date, 1);
    default:
      return null;
  }
}

/** The same day number in the neighbouring month, clamped to that month's last day. */
export function shiftMonthKeepingDay(date: string, delta: number): string {
  const [, , d] = parseYmd(date);
  const month = shiftMonth(date.slice(0, 7), delta);
  const [y, m] = parseYmd(month);
  const last = parseYmd(addDays(`${shiftMonth(month, 1)}-01`, -1))[2];
  return formatYmd(y, m, Math.min(d, last));
}

/** A range being chosen: `to` is null until the second pick. */
export interface DateRange {
  from: string | null;
  to: string | null;
}

export const EMPTY_RANGE: DateRange = { from: null, to: null };

/**
 * The range after picking `date`. The first pick starts one; a second pick completes it (an
 * earlier day swaps the ends, the same day makes a one-day range); a pick after that starts over.
 */
export function pickRange(range: DateRange, date: string): DateRange {
  if (range.from === null || range.to !== null) return { from: date, to: null };
  return date < range.from ? { from: date, to: range.from } : { from: range.from, to: date };
}

/** True when `date` is inside the range, ends included. A half-chosen range holds only `from`. */
export function inRange(range: DateRange, date: string): boolean {
  if (range.from === null) return false;
  return date >= range.from && date <= (range.to ?? range.from);
}
