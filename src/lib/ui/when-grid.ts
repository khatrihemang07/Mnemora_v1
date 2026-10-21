/**
 * The When picker's calendar model: a rolling five-week grid that starts at this week, the quick
 * days, and the labels the picker shows. Pure, so it is unit-tested. A date is `YYYY-MM-DD` and has
 * no zone. Past days are disabled; nothing before this week is ever shown.
 */

import { addDays, parseYmd, type WeekStart, weekdayLabels } from './calendar';

/** Weeks in the grid, and so the days a ‹ › press moves by (times seven). */
export const GRID_WEEKS = 5;
const GRID_DAYS = GRID_WEEKS * 7;

const DAY_MS = 86_400_000;
const WEEKDAYS = ['Sun', 'Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat'];
const MONTHS = ['Jan', 'Feb', 'Mar', 'Apr', 'May', 'Jun', 'Jul', 'Aug', 'Sep', 'Oct', 'Nov', 'Dec'];
const MONTHS_LONG = [
  'January',
  'February',
  'March',
  'April',
  'May',
  'June',
  'July',
  'August',
  'September',
  'October',
  'November',
  'December',
];

/** Whole days from `a` to `b` (negative when `b` is earlier). */
export function daysBetween(a: string, b: string): number {
  const [ya = 1970, ma = 1, da = 1] = parseYmd(a);
  const [yb = 1970, mb = 1, db = 1] = parseYmd(b);
  return Math.round((Date.UTC(yb, mb - 1, db) - Date.UTC(ya, ma - 1, da)) / DAY_MS);
}

/** 0 (Sunday) to 6 (Saturday). */
export function weekdayOf(date: string): number {
  const [y = 1970, m = 1, d = 1] = parseYmd(date);
  return new Date(Date.UTC(y, m - 1, d)).getUTCDay();
}

/** "Mon", "Tue", … */
export const weekdayShort = (date: string): string => WEEKDAYS[weekdayOf(date)] ?? '';

/** "Oct", "Nov", … for a `YYYY-MM-DD` date. */
export const monthShort = (date: string): string => MONTHS[parseYmd(date)[1] - 1] ?? '';

/** The first day of the week that holds `date`. */
export function weekStartOf(date: string, weekStart: WeekStart): string {
  const offset = (weekdayOf(date) - (weekStart === 'sun' ? 0 : 1) + 7) % 7;
  return addDays(date, -offset);
}

export interface GridCell {
  /** `YYYY-MM-DD`. */
  date: string;
  day: number;
  /** Before today: not pickable. */
  past: boolean;
  today: boolean;
  /** "Nov" on the 1st of a month, else null: the tiny label that shows where months change. */
  monthLabel: string | null;
}

export interface GridModel {
  /** Seven single-letter headings, in grid order. */
  weekdays: string[];
  /** 35 days, row by row. */
  cells: GridCell[];
  /** First and last day shown. */
  start: string;
  end: string;
  /** "October 2026", "Oct – Nov 2026" or "Dec 2026 – Jan 2027". */
  heading: string;
  /** False on the first page: there is nothing before this week. */
  canGoBack: boolean;
}

/** Page `page` (0 is the one that starts at this week) of the rolling grid. */
export function buildGrid(today: string, weekStart: WeekStart, page: number): GridModel {
  const index = Math.max(0, Math.floor(page));
  const start = addDays(weekStartOf(today, weekStart), index * GRID_DAYS);
  const cells = Array.from({ length: GRID_DAYS }, (_, i): GridCell => {
    const date = addDays(start, i);
    const day = parseYmd(date)[2];
    return {
      date,
      day,
      past: date < today,
      today: date === today,
      monthLabel: day === 1 ? monthShort(date) : null,
    };
  });
  const end = addDays(start, GRID_DAYS - 1);
  const [y1, m1] = parseYmd(start);
  const [y2, m2] = parseYmd(end);
  const heading =
    y1 === y2 && m1 === m2
      ? `${MONTHS_LONG[m1 - 1]} ${y1}`
      : y1 === y2
        ? `${MONTHS[m1 - 1]} – ${MONTHS[m2 - 1]} ${y1}`
        : `${MONTHS[m1 - 1]} ${y1} – ${MONTHS[m2 - 1]} ${y2}`;
  return {
    weekdays: weekdayLabels(weekStart).map((l) => l.slice(0, 1)),
    cells,
    start,
    end,
    heading,
    canGoBack: index > 0,
  };
}

/** The page of the grid that shows `date`; the first page for today and for anything earlier. */
export function pageForDate(date: string, today: string, weekStart: WeekStart): number {
  const from = weekStartOf(today, weekStart);
  return Math.max(0, Math.floor(daysBetween(from, date) / GRID_DAYS));
}

export interface QuickDay {
  key: 'today' | 'tomorrow' | 'weekend' | 'next_week';
  label: string;
  date: string;
  /** The weekday shown on the right: "Wed". */
  hint: string;
}

/**
 * The quick choices. Weekend is the current week's closest weekend day: today on a Saturday or
 * Sunday, otherwise the coming Saturday. Next week is the first day of next week, which depends on
 * the Week start Preference.
 */
export function quickDays(today: string, weekStart: WeekStart): QuickDay[] {
  const wd = weekdayOf(today);
  const weekend = wd === 0 || wd === 6 ? today : addDays(today, 6 - wd);
  const nextWeek = addDays(weekStartOf(today, weekStart), 7);
  const day = (key: QuickDay['key'], label: string, date: string): QuickDay => ({
    key,
    label,
    date,
    hint: weekdayShort(date),
  });
  return [
    day('today', 'Today', today),
    day('tomorrow', 'Tomorrow', addDays(today, 1)),
    day('weekend', 'Weekend', weekend),
    day('next_week', 'Next week', nextWeek),
  ];
}

/** "Fri 23 Oct", with the year when it is not `today`'s year: a date named in a sentence or chip. */
export function dayText(date: string, today: string): string {
  const [y, m, d] = parseYmd(date);
  const year = y === parseYmd(today)[0] ? '' : ` ${y}`;
  return `${weekdayShort(date)} ${d} ${MONTHS[m - 1]}${year}`;
}
