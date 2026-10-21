/**
 * Repeat rules for the When picker and the browser mock: a TypeScript copy of the engine in
 * crates/todo/src/recurrence.rs, kept honest by the shared fixtures
 * (src/lib/ipc/fixtures/recurrence-rules.json). Dates are `YYYY-MM-DD` and have no zone. A rule
 * counts from the Due date (`fixed`) or from the day it is done (`after`). Weeks run Monday to
 * Sunday here, which only shows for "every N weeks" on chosen weekdays.
 */

import type { RepeatRule } from '../ipc/bindings';
import { addDays, formatYmd, parseYmd } from './calendar';
import { monthShort, weekdayOf } from './when-grid';

export type { RepeatRule };

/** Largest interval: "every 99 days". */
export const MAX_EVERY = 99;
const ALL_DAYS = 0b111_1111;
/** Monday to Friday, Sunday being bit 0. */
export const WEEKDAYS_MASK = 0b011_1110;
const MAX_YEAR = 9999;

export type RepeatFreq = 'day' | 'week' | 'month' | 'year';

/** Short weekday names by day number, Sunday first. */
const DAY_NAMES = ['Sun', 'Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat'];
/** The order weekdays are listed and toggled in. */
export const WEEK_ORDER = [1, 2, 3, 4, 5, 6, 0] as const;

/** The bit for a weekday (0 is Sunday). */
export const dayBit = (weekday: number): number => 1 << weekday;

/** The weekdays in a mask, Monday first. */
export const maskDays = (mask: number): number[] => WEEK_ORDER.filter((d) => mask & dayBit(d));

/** Why a rule is not acceptable, in the words Rust uses, or null when it is. Mirrors `check`. */
export function ruleProblem(rule: RepeatRule): string | null {
  if (!['day', 'week', 'month', 'year'].includes(rule.freq)) {
    return 'Repeat by days, weeks, months or years';
  }
  if (!Number.isInteger(rule.every) || rule.every < 1 || rule.every > MAX_EVERY) {
    return 'Repeat every 1 to 99';
  }
  if (rule.mode !== 'fixed' && rule.mode !== 'after') {
    return "Count from the due date or from the day it's done";
  }
  const days = rule.days ?? 0;
  if (!Number.isInteger(days) || days < 0) return 'Not a valid repeat day';
  if (rule.freq === 'week' && days > ALL_DAYS) return 'Not a valid repeat day';
  if (rule.freq === 'week' && rule.mode === 'after' && days !== 0) {
    return "From the day it's done needs an interval, not weekdays";
  }
  if ((rule.freq === 'month' || rule.freq === 'year') && days > 31) return 'Not a valid repeat day';
  return null;
}

/** The rule as it is stored: days mean nothing for a day interval or an "after done" rule. */
export function settleRule(rule: RepeatRule): RepeatRule {
  const days = rule.days ?? 0;
  const keep =
    rule.freq === 'week'
      ? days
      : (rule.freq === 'month' || rule.freq === 'year') && rule.mode === 'fixed'
        ? days
        : 0;
  return { freq: rule.freq, every: rule.every, days: keep === 0 ? null : keep, mode: rule.mode };
}

/** A fixed month or year rule keeps to the day of the month of its Due date. */
export function anchoredOn(rule: RepeatRule, due: string): RepeatRule {
  if (rule.mode === 'fixed' && (rule.freq === 'month' || rule.freq === 'year')) {
    return { ...rule, days: parseYmd(due)[2] };
  }
  return rule;
}

const daysInMonth = (year: number, month: number): number =>
  new Date(Date.UTC(year, month, 0)).getUTCDate();

/** The date in `year`-`month` on `day`, clamped to the month's last day; null past year 9999. */
function clamped(year: number, month: number, day: number): string | null {
  if (year < 1 || year > MAX_YEAR) return null;
  return formatYmd(year, month, Math.min(day, daysInMonth(year, month)));
}

/** The date one step after `date`, strictly later; null past the year 9999. */
export function stepDate(rule: RepeatRule, date: string): string | null {
  const every = rule.every;
  const days = rule.days ?? 0;
  let next: string | null;
  if (rule.freq === 'day') {
    next = addDays(date, every);
  } else if (rule.freq === 'week') {
    if (days === 0) {
      next = addDays(date, 7 * every);
    } else {
      // The week runs Monday (0) to Sunday (6): chosen days later in this week come first, then
      // the week `every` weeks on.
      const from = (weekdayOf(date) + 6) % 7;
      const hit = (offset: number) => (days & dayBit((offset + 1) % 7)) !== 0;
      let offset = -1;
      for (let o = from + 1; o < 7 && offset < 0; o++) if (hit(o)) offset = o;
      if (offset >= 0) {
        next = addDays(date, offset - from);
      } else {
        for (let o = 0; o < 7 && offset < 0; o++) if (hit(o)) offset = o;
        next = addDays(date, 7 * every - from + offset);
      }
    }
  } else if (rule.freq === 'month') {
    const [y, m, d] = parseYmd(date);
    const months = y * 12 + m - 1 + every;
    next = clamped(Math.floor(months / 12), (months % 12) + 1, days > 0 ? days : d);
  } else {
    const [y, m, d] = parseYmd(date);
    next = clamped(y + every, m, days > 0 ? days : d);
  }
  return next !== null && /^\d{4}-\d{2}-\d{2}$/.test(next) ? next : null;
}

/** The first day on or after `today` the rule lands on; only chosen weekdays make it later. */
export function firstMatch(rule: RepeatRule, today: string): string {
  const days = rule.days ?? 0;
  if (rule.freq !== 'week' || days === 0) return today;
  for (let i = 0; i < 7; i++) {
    const date = addDays(today, i);
    if (days & dayBit(weekdayOf(date))) return date;
  }
  return today;
}

/**
 * Where a Task due `due` goes when it is done on `today`: from the day it is done in "after" mode;
 * otherwise the first date on the schedule after today (one step after the Due date when that is
 * still ahead, skipping the missed ones when late). Null past the year 9999.
 */
export function nextDue(rule: RepeatRule, due: string, today: string): string | null {
  if (rule.mode === 'after') return stepDate({ ...rule, days: null }, today);
  let date = stepDate(rule, due);
  while (date !== null && date <= today) date = stepDate(rule, date);
  return date;
}

/** The next `count` dates after `from`, each a step on the last: the picker's "Next:" line. */
export function preview(rule: RepeatRule, from: string, count = 3): string[] {
  const out: string[] = [];
  let date: string | null = from;
  while (out.length < count && date !== null) {
    date = stepDate(rule, date);
    if (date !== null) out.push(date);
  }
  return out;
}

/** The rule's dates after `from` up to `to`, both included: the rings on the calendar. */
export function datesBetween(rule: RepeatRule, from: string, start: string, to: string): string[] {
  const out: string[] = [];
  let date: string | null = from;
  for (let i = 0; i < 4000 && date !== null; i++) {
    date = stepDate(rule, date);
    if (date === null || date > to) break;
    if (date >= start) out.push(date);
  }
  return out;
}

/** 1, 2, 3 → "1st", "2nd", "3rd"; 11 → "11th". */
export function ordinal(n: number): string {
  const tens = n % 100;
  if (tens >= 11 && tens <= 13) return `${n}th`;
  return `${n}${['th', 'st', 'nd', 'rd'][n % 10] ?? 'th'}`;
}

/** The weekday names of a mask, Monday first: "Mon, Thu". */
const dayList = (mask: number): string =>
  maskDays(mask)
    .map((d) => DAY_NAMES[d])
    .join(', ');

/** The short rule shown on a row and in the Repeat chip: "Every 3 days after done". */
export function repeatLabel(rule: RepeatRule): string {
  const days = rule.days ?? 0;
  const n = rule.every;
  const unit = { day: 'day', week: 'week', month: 'month', year: 'year' }[rule.freq] ?? rule.freq;
  let text = n === 1 ? `Every ${unit}` : `Every ${n} ${unit}s`;
  if (rule.freq === 'week' && days !== 0) {
    text =
      n === 1 && days === WEEKDAYS_MASK
        ? 'Every weekday'
        : `${n === 1 ? 'Every' : `Every ${n} weeks on`} ${dayList(days)}`;
  }
  return rule.mode === 'after' ? `${text} after done` : text;
}

export interface RepeatPreset {
  key: string;
  label: string;
  rule: RepeatRule;
}

const WEEKDAY_NAMES = [
  'Sunday',
  'Monday',
  'Tuesday',
  'Wednesday',
  'Thursday',
  'Friday',
  'Saturday',
];

const preset = (
  key: string,
  label: string,
  freq: RepeatFreq,
  every: number,
  days: number | null,
): RepeatPreset => ({ key, label, rule: { freq, every, days, mode: 'fixed' } });

/** The presets, worded for `date` (the Task's Due date, or the day it would start on). */
export function repeatPresets(date: string): RepeatPreset[] {
  const d = parseYmd(date)[2];
  return [
    preset('day', 'Every day', 'day', 1, null),
    preset('d2', 'Every 2 days', 'day', 2, null),
    preset('d3', 'Every 3 days', 'day', 3, null),
    preset('d4', 'Every 4 days', 'day', 4, null),
    preset('weekday', 'Every weekday', 'week', 1, WEEKDAYS_MASK),
    preset('week', `Weekly on ${WEEKDAY_NAMES[weekdayOf(date)]}`, 'week', 1, null),
    preset('month', `Monthly on the ${ordinal(d)}`, 'month', 1, null),
    preset('year', `Yearly on ${d} ${monthShort(date)}`, 'year', 1, null),
  ];
}

/** Which preset a rule is, or null for a custom one. The mode does not matter. */
export function presetKey(rule: RepeatRule | null): string | null {
  if (!rule) return null;
  const days = rule.days ?? 0;
  if (rule.freq === 'day')
    return rule.every === 1 ? 'day' : rule.every <= 4 ? `d${rule.every}` : null;
  if (rule.every !== 1) return null;
  if (rule.freq === 'week') return days === WEEKDAYS_MASK ? 'weekday' : days === 0 ? 'week' : null;
  return rule.freq;
}

/** "From the day it's done" is offered only with an interval, not chosen weekdays. */
export const afterAllowed = (rule: RepeatRule): boolean =>
  !(rule.freq === 'week' && (rule.days ?? 0) !== 0);
