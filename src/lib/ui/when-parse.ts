/**
 * The When picker's "Type a date" field. A deliberately small reading of typed text ("fri", "28
 * oct", "in 3 days", "tomorrow 9am"): the full natural-language parser arrives with capture and
 * plugs in through the `parse` prop (`WhenParser`). Anything it cannot read gives null, and the
 * field says so rather than guessing.
 */

import { addDays, type WeekStart } from './calendar';
import { quickDays, weekdayOf, weekStartOf } from './when-grid';
import { timeSuggestions } from './when-time';

/** What the reader needs to know about the moment. */
export interface WhenContext {
  /** `YYYY-MM-DD`. */
  today: string;
  /** `HH:MM`. */
  now: string;
  weekStart: WeekStart;
}

/** A reading: a date, a time, or both. Neither is never returned. */
export interface ParsedWhen {
  /** `YYYY-MM-DD`; null when only a time was typed. */
  date: string | null;
  /** `HH:MM`. */
  time: string | null;
}

/** Turns typed text into a When, or null when it cannot. */
export type WhenParser = (text: string, ctx: WhenContext) => ParsedWhen | null;

const WEEKDAYS: Record<string, number> = {
  sun: 0,
  sunday: 0,
  mon: 1,
  monday: 1,
  tue: 2,
  tues: 2,
  tuesday: 2,
  wed: 3,
  wednesday: 3,
  thu: 4,
  thur: 4,
  thurs: 4,
  thursday: 4,
  fri: 5,
  friday: 5,
  sat: 6,
  saturday: 6,
};

const MONTHS: Record<string, number> = {
  jan: 1,
  january: 1,
  feb: 2,
  february: 2,
  mar: 3,
  march: 3,
  apr: 4,
  april: 4,
  may: 5,
  jun: 6,
  june: 6,
  jul: 7,
  july: 7,
  aug: 8,
  august: 8,
  sep: 9,
  sept: 9,
  september: 9,
  oct: 10,
  october: 10,
  nov: 11,
  november: 11,
  dec: 12,
  december: 12,
};

const TIME = String.raw`noon|midnight|\d{1,2}(?::\d{2})?\s*(?:am|pm|a|p)|\d{1,2}:\d{2}`;
const TIME_AT_END = new RegExp(String.raw`(?:^|\s)(?:at\s+)?(${TIME})$`);
const TIME_AT_START = new RegExp(String.raw`^(?:at\s+)?(${TIME})(?:\s+|$)`);

/** Splits a time off either end of the text. */
function takeTime(text: string): { rest: string; time: string | null } {
  for (const [re, cut] of [
    [TIME_AT_END, (m: RegExpMatchArray) => text.slice(0, m.index)],
    [TIME_AT_START, (m: RegExpMatchArray) => text.slice(m[0].length)],
  ] as const) {
    const m = text.match(re);
    const time = m?.[1] ? timeSuggestions(m[1])[0] : undefined;
    if (m && time) return { rest: cut(m).trim(), time };
  }
  return { rest: text, time: null };
}

function isRealDate(y: number, m: number, d: number): boolean {
  const at = new Date(Date.UTC(y, m - 1, d));
  return at.getUTCFullYear() === y && at.getUTCMonth() === m - 1 && at.getUTCDate() === d;
}

const ymd = (y: number, m: number, d: number) =>
  `${String(y).padStart(4, '0')}-${String(m).padStart(2, '0')}-${String(d).padStart(2, '0')}`;

/**
 * A day and month, with an optional year. Without a year it is the next time that day comes round
 * (a past one means next year; 29 Feb waits for a leap year).
 */
function dayMonth(day: number, month: number, year: number | null, today: string): string | null {
  if (year !== null) {
    return isRealDate(year, month, day) && ymd(year, month, day) >= today
      ? ymd(year, month, day)
      : null;
  }
  const thisYear = Number(today.slice(0, 4));
  for (let y = thisYear; y <= thisYear + 8; y++) {
    if (isRealDate(y, month, day) && ymd(y, month, day) >= today) return ymd(y, month, day);
  }
  return null;
}

function readDate(text: string, ctx: WhenContext): string | null {
  const { today, weekStart } = ctx;
  if (text === 'today' || text === 'tod') return today;
  if (['tomorrow', 'tmr', 'tmrw', 'tom'].includes(text)) return addDays(today, 1);
  if (text === 'next week') return addDays(weekStartOf(today, weekStart), 7);
  if (['weekend', 'this weekend'].includes(text)) {
    return quickDays(today, weekStart).find((d) => d.key === 'weekend')?.date ?? null;
  }

  const weekday = text.match(/^(?:on\s+|this\s+)?(next\s+)?([a-z]+)$/);
  const index = weekday?.[2] ? WEEKDAYS[weekday[2]] : undefined;
  if (weekday && index !== undefined) {
    if (weekday[1]) {
      // "next fri" is the Friday of next week.
      const inWeek = (index - (weekStart === 'sun' ? 0 : 1) + 7) % 7;
      return addDays(weekStartOf(today, weekStart), 7 + inWeek);
    }
    return addDays(today, (index - weekdayOf(today) + 7) % 7);
  }

  const relative = text.match(/^in\s+(\d{1,3})\s*(d|days?|w|weeks?)$/);
  if (relative?.[1] && relative[2]) {
    return addDays(today, Number(relative[1]) * (relative[2].startsWith('w') ? 7 : 1));
  }

  const iso = text.match(/^(\d{4})-(\d{2})-(\d{2})$/);
  if (iso) {
    const [, y, m, d] = iso.map(Number) as [number, number, number, number];
    return isRealDate(y, m, d) && ymd(y, m, d) >= today ? ymd(y, m, d) : null;
  }

  // "28 oct", "28 october 2027", "oct 28", "oct 28 2027". Numeric forms like 2/3 are never read.
  const dm = text.match(/^(\d{1,2})(?:st|nd|rd|th)?\s+([a-z]+)(?:\s+(\d{4}))?$/);
  const md = text.match(/^([a-z]+)\s+(\d{1,2})(?:st|nd|rd|th)?(?:\s+(\d{4}))?$/);
  const [dayText, monthText, yearText] = dm
    ? [dm[1], dm[2], dm[3]]
    : md
      ? [md[2], md[1], md[3]]
      : [];
  const month = monthText ? MONTHS[monthText] : undefined;
  if (dayText && month) {
    return dayMonth(Number(dayText), month, yearText ? Number(yearText) : null, today);
  }
  return null;
}

/** The default reader. */
export const simpleWhenParser: WhenParser = (input, ctx) => {
  const text = input.trim().toLowerCase().replace(/\s+/g, ' ');
  if (!text) return null;
  if (text === 'tonight') return { date: ctx.today, time: '20:00' };
  const { rest, time } = takeTime(text);
  if (rest === '') return time ? { date: null, time } : null;
  const date = readDate(rest, ctx);
  return date ? { date, time } : null;
};
