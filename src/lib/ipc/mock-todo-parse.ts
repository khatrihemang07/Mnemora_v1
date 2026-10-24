/**
 * The browser mock's copy of the capture parser in crates/todo/src/parse.rs: reads dates, times,
 * Repeats, `#List` and `p1`-`p4` out of typed text. Pure; the caller says what today and now are.
 * Both run src/lib/ipc/fixtures/parse-cases.json (ADR 0009), so keep the two in step. Offsets are
 * UTF-16 units, which is what a JavaScript string counts in anyway.
 */

import { addDays } from '../ui/calendar';
import type { ParseAlternative, Parsed, ParseRequest, ParseSpan, RepeatRule } from './bindings';

/** A List `#Name` may match, in the List order. */
export interface ListRef {
  id: string;
  name: string;
}

const WEEKDAYS_MASK = 0b011_1110;
const TONIGHT = '20:00';
const KINDS = ['repeat', 'date', 'time', 'prio', 'list'] as const;
const MAX_LIST_NAME_CHARS = 60;

type Meaning =
  | { kind: 'date'; value: string }
  | { kind: 'time'; value: string }
  | { kind: 'repeat'; rule: RepeatRule }
  | { kind: 'prio'; value: number }
  | { kind: 'list'; id: string | null; name: string };

interface Cand {
  start: number;
  end: number;
  word: string;
  meaning: Meaning;
  key: string;
}

interface Token {
  from: number;
  to: number;
  low: string;
  raw: string;
  comma: boolean;
  masked: boolean;
}

/** A recogniser's answer: tokens used, and what they set. */
type Match = [number, Meaning[]];

const meaningValue = (m: Meaning): string => {
  switch (m.kind) {
    case 'date':
    case 'time':
      return m.value;
    case 'repeat':
      // Same field order as serde, so the value is the same text in both.
      return JSON.stringify({
        freq: m.rule.freq,
        every: m.rule.every,
        days: m.rule.days ?? null,
        mode: m.rule.mode,
      });
    case 'prio':
      return String(m.value);
    case 'list':
      return m.name;
  }
};

const toSpan = (c: Cand): ParseSpan => ({
  key: c.key,
  kind: c.meaning.kind,
  start: c.start,
  end: c.end,
  word: c.word,
  value: meaningValue(c.meaning),
  new_list: c.meaning.kind === 'list' && c.meaning.id === null,
});

function maskRanges(text: string): [number, number][] {
  const out: [number, number][] = [];
  let i = 0;
  while (i < text.length) {
    const open = text[i];
    const close = open === '"' ? '"' : open === '“' ? '”' : open === '`' ? '`' : null;
    if (close === null) {
      i++;
      continue;
    }
    const at = text.indexOf(close, i + 1);
    if (at < 0) {
      i++;
      continue;
    }
    out.push([i, at + 1]);
    i = at + 1;
  }
  return out;
}

const isSpace = (c: string) => /\s/.test(c);

function tokenize(text: string): Token[] {
  const masks = maskRanges(text);
  const out: Token[] = [];
  let i = 0;
  while (i < text.length) {
    if (isSpace(text[i] as string)) {
      i++;
      continue;
    }
    const a = i;
    while (i < text.length && !isSpace(text[i] as string)) i++;
    let from = a;
    let to = i;
    while (from < to && '([{'.includes(text[from] as string)) from++;
    let comma = false;
    while (to > from && '.,;:!?)]}'.includes(text[to - 1] as string)) {
      comma ||= text[to - 1] === ',';
      to--;
    }
    if (from === to) continue;
    const raw = text.slice(from, to);
    out.push({
      from,
      to,
      raw,
      low: raw.toLowerCase(),
      comma,
      masked: masks.some(([s, e]) => a < e && i > s),
    });
  }
  return out;
}

const WEEKDAY_NAMES: Record<string, number> = {
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
const weekdayName = (w: string | undefined): number | null =>
  w !== undefined && Object.hasOwn(WEEKDAY_NAMES, w) ? (WEEKDAY_NAMES[w] as number) : null;

const MONTH_NAMES: Record<string, number> = {
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
const monthName = (w: string | undefined): number | null =>
  w !== undefined && Object.hasOwn(MONTH_NAMES, w) ? (MONTH_NAMES[w] as number) : null;

/** A number written with plain digits, at most `maxLen` of them. */
const digits = (w: string | undefined, maxLen: number): number | null =>
  w !== undefined && w.length > 0 && w.length <= maxLen && /^\d+$/.test(w) ? Number(w) : null;

const ymdParts = (d: string): [number, number, number] => [
  Number(d.slice(0, 4)),
  Number(d.slice(5, 7)),
  Number(d.slice(8, 10)),
];

const isRealDate = (y: number, m: number, d: number): boolean => {
  const at = new Date(Date.UTC(y, m - 1, d));
  at.setUTCFullYear(y);
  return at.getUTCFullYear() === y && at.getUTCMonth() === m - 1 && at.getUTCDate() === d;
};

const weekdayOf = (date: string): number => {
  const [y, m, d] = ymdParts(date);
  return new Date(Date.UTC(y, m - 1, d)).getUTCDay();
};

interface Context {
  today: string;
  /** 1 for Monday, 0 for Sunday. */
  weekStart: number;
  lists: ListRef[];
  fold: (s: string) => string;
}

const startOfWeek = (cx: Context, date: string): string =>
  addDays(date, -((weekdayOf(date) + 7 - cx.weekStart) % 7));

/** A day and month, with a year or without: the next time that day comes round. */
function namedDate(cx: Context, day: number, month: number, year: number | null): string | null {
  const make = (y: number): string | null =>
    isRealDate(y, month, day)
      ? `${String(y).padStart(4, '0')}-${String(month).padStart(2, '0')}-${String(day).padStart(2, '0')}`
      : null;
  if (year !== null) {
    const d = make(year);
    return d !== null && d >= cx.today ? d : null;
  }
  const thisYear = ymdParts(cx.today)[0];
  for (let y = thisYear; y < thisYear + 8; y++) {
    const d = make(y);
    if (d !== null && d >= cx.today) return d;
  }
  return null;
}

function nextMonth(cx: Context): string {
  const [y, m] = ymdParts(cx.today);
  const [ny, nm] = m === 12 ? [y + 1, 1] : [y, m + 1];
  return `${String(ny).padStart(4, '0')}-${String(nm).padStart(2, '0')}-01`;
}

const unitOf = (w: string): RepeatRule['freq'] | null => {
  const u = w.endsWith('s') ? w.slice(0, -1) : w;
  return u === 'day' || u === 'week' || u === 'month' || u === 'year' ? u : null;
};

/** The rule when it is acceptable; days mean nothing for a day interval. */
function makeRule(
  freq: RepeatRule['freq'],
  every: number,
  days: number,
  after: boolean,
): RepeatRule | null {
  if (!(every >= 1 && every <= 99)) return null;
  const week = freq === 'week';
  if (after && week && days !== 0) return null;
  return {
    freq,
    every,
    days: week && days !== 0 ? days : null,
    mode: after ? 'after' : 'fixed',
  };
}

function readRepeat(t: Token[], i: number): Match | null {
  const at = (n: number) => t[n]?.low;
  let used: number;
  let parts: [RepeatRule['freq'], number, number];
  const first = at(i);
  if (first === 'daily') [used, parts] = [1, ['day', 1, 0]];
  else if (first === 'weekly') [used, parts] = [1, ['week', 1, 0]];
  else if (first === 'monthly') [used, parts] = [1, ['month', 1, 0]];
  else if (first === 'yearly') [used, parts] = [1, ['year', 1, 0]];
  else if (first === 'every') {
    const word = at(i + 1);
    const next = at(i + 2);
    if (word === undefined) return null;
    const n = digits(word, 2);
    if (word === 'weekday' || word === 'weekdays') {
      [used, parts] = [2, ['week', 1, WEEKDAYS_MASK]];
    } else if (word === 'other' && next !== undefined) {
      const unit = unitOf(next);
      if (unit === null) return null;
      [used, parts] = [3, [unit, 2, 0]];
    } else if (n !== null && next !== undefined) {
      const unit = unitOf(next);
      if (unit === null) return null;
      [used, parts] = [3, [unit, n, 0]];
    } else if (unitOf(word) !== null && !word.endsWith('s')) {
      [used, parts] = [2, [unitOf(word) as RepeatRule['freq'], 1, 0]];
    } else {
      const w = weekdayName(word);
      if (w === null) return null;
      let mask = 1 << w;
      let k = i + 1;
      for (;;) {
        let nx = k + 1;
        let separated = t[k]?.comma === true;
        if (at(nx) === 'and' || at(nx) === '&') {
          nx++;
          separated = true;
        }
        const wd = weekdayName(at(nx));
        if (wd === null || !separated) break;
        mask |= 1 << wd;
        k = nx;
      }
      [used, parts] = [k + 1 - i, ['week', 1, mask]];
    }
  } else return null;
  // "After done" counts from an interval, never from chosen weekdays.
  const after = at(i + used) === 'after' && at(i + used + 1) === 'done' && parts[2] === 0;
  if (after) used += 2;
  const rule = makeRule(parts[0], parts[1], parts[2], after);
  return rule ? [used, [{ kind: 'repeat', rule }]] : null;
}

function readDate(t: Token[], i: number, cx: Context): Match | null {
  const at = (n: number) => t[n]?.low;
  const today = cx.today;
  const date = (used: number, d: string): Match => [used, [{ kind: 'date', value: d }]];
  const first = at(i);
  if (first === undefined) return null;
  const untilWeekday = (w: number) => (w + 7 - weekdayOf(today)) % 7;
  switch (first) {
    case 'today':
    case 'tod':
      return date(1, today);
    case 'tomorrow':
    case 'tmr':
    case 'tmrw':
    case 'tom':
      return date(1, addDays(today, 1));
    case 'tonight':
      return [
        1,
        [
          { kind: 'date', value: today },
          { kind: 'time', value: TONIGHT },
        ],
      ];
    case 'on': {
      const w = weekdayName(at(i + 1));
      return w === null ? null : date(2, addDays(today, untilWeekday(w)));
    }
    case 'next': {
      const nextWeek = addDays(startOfWeek(cx, today), 7);
      const word = at(i + 1);
      if (word === 'week') return date(2, nextWeek);
      if (word === 'month') return date(2, nextMonth(cx));
      const w = weekdayName(word);
      return w === null ? null : date(2, addDays(nextWeek, (w + 7 - cx.weekStart) % 7));
    }
    case 'this': {
      if (at(i + 1) !== 'weekend') break;
      const wd = weekdayOf(today);
      return date(2, addDays(today, wd === 0 || wd === 6 ? 0 : 6 - wd));
    }
    case 'in': {
      const n = digits(at(i + 1), 3);
      const unitWord = at(i + 2);
      const unit = unitWord === undefined ? null : unitOf(unitWord);
      if (n === null || n === 0 || (unit !== 'day' && unit !== 'week')) return null;
      return date(3, addDays(today, n * (unit === 'week' ? 7 : 1)));
    }
    default:
  }
  const wd = weekdayName(first);
  if (wd !== null) return date(1, addDays(today, untilWeekday(wd)));
  // "21 oct", "21st october 2027" and "oct 21", "oct 21st 2027".
  const dayOf = (word: string | undefined): number | null => {
    const m = word === undefined ? null : /^(\d{1,2})(st|nd|rd|th)?$/.exec(word);
    const d = m ? Number(m[1]) : null;
    return d !== null && d >= 1 && d <= 31 ? d : null;
  };
  let day: number | null;
  let month: number | null;
  const firstDay = dayOf(first);
  if (firstDay !== null) {
    day = firstDay;
    month = monthName(at(i + 1));
  } else {
    month = monthName(first);
    day = dayOf(at(i + 1));
  }
  if (day === null || month === null) return null;
  const yearWord = at(i + 2);
  const year = yearWord !== undefined && yearWord.length === 4 ? digits(yearWord, 4) : null;
  const found = namedDate(cx, day, month, year);
  return found === null ? null : date(year === null ? 2 : 3, found);
}

const clock = (h: number, m: number): string | null =>
  h < 24 && m < 60 ? `${String(h).padStart(2, '0')}:${String(m).padStart(2, '0')}` : null;

const twelve = (h: number, m: number, pm: boolean): string | null =>
  h >= 1 && h <= 12 ? clock((h % 12) + (pm ? 12 : 0), m) : null;

/** `9` or `9:30`. */
function hourMinute(word: string): [number, number] | null {
  const colon = word.indexOf(':');
  if (colon < 0) {
    const h = digits(word, 2);
    return h === null ? null : [h, 0];
  }
  const m = word.slice(colon + 1);
  const h = digits(word.slice(0, colon), 2);
  const mm = m.length === 2 ? digits(m, 2) : null;
  return h === null || mm === null ? null : [h, mm];
}

function readTime(t: Token[], i: number): Match | null {
  const at = (n: number) => t[n]?.low;
  const atPrefix = at(i) === 'at';
  const k = i + (atPrefix ? 1 : 0);
  const used = (n: number) => n + (atPrefix ? 1 : 0);
  const found = (n: number, time: string | null): Match | null =>
    time === null ? null : [used(n), [{ kind: 'time', value: time }]];
  const word = at(k);
  if (word === undefined) return null;
  if (word === 'noon') return found(1, '12:00');
  if (word === 'midnight') return found(1, '00:00');
  const mer = word.endsWith('am') ? 'am' : word.endsWith('pm') ? 'pm' : null;
  if (mer) {
    const hm = hourMinute(word.slice(0, -2));
    if (hm) return found(1, twelve(hm[0], hm[1], mer === 'pm'));
  }
  const hm = hourMinute(word);
  const next = at(k + 1);
  if (hm && (next === 'am' || next === 'pm')) return found(2, twelve(hm[0], hm[1], next === 'pm'));
  if (hm && word.includes(':')) return found(1, clock(hm[0], hm[1]));
  // A bare hour is a time only after "at", and not when a month follows ("at 21 oct").
  if (atPrefix && monthName(at(k + 1)) === null) {
    const h = digits(word, 2);
    return h === null ? null : found(1, clock(h, 0));
  }
  return null;
}

function readPrio(t: Token[], i: number): Match | null {
  const m = /^p([1-4])$/.exec(t[i]?.low ?? '');
  return m ? [1, [{ kind: 'prio', value: Number(m[1]) }]] : null;
}

const TAG = /^[\p{L}\p{N}_-]+$/u;

/** The part of a List name a `#tag` can spell: letters, digits, `_` and `-`, folded. */
const tagKey = (cx: Context, name: string): string =>
  [...cx.fold(name)].filter((c) => /[\p{L}\p{N}_-]/u.test(c)).join('');

function readList(t: Token[], i: number, cx: Context): Match | null {
  const raw = t[i]?.raw ?? '';
  if (!raw.startsWith('#')) return null;
  const name = raw.slice(1);
  if (!name || !TAG.test(name)) return null;
  const typed = tagKey(cx, name);
  const hit =
    cx.lists.find((l) => tagKey(cx, l.name) === typed) ??
    cx.lists.find((l) => tagKey(cx, l.name).startsWith(typed));
  if (hit) return [1, [{ kind: 'list', id: hit.id, name: hit.name }]];
  // A tag of digits only (an issue number) is a reference, not a List to create.
  if (/^\d+$/.test(name) || [...name].length > MAX_LIST_NAME_CHARS) return null;
  return [1, [{ kind: 'list', id: null, name }]];
}

function candidates(text: string, cx: Context): Cand[] {
  const tokens = tokenize(text);
  const out: Cand[] = [];
  const seen = new Map<string, number>();
  let i = 0;
  while (i < tokens.length) {
    let found: Match | null = null;
    if (!tokens[i]?.masked) {
      found =
        readRepeat(tokens, i) ??
        readDate(tokens, i, cx) ??
        readTime(tokens, i) ??
        readPrio(tokens, i) ??
        readList(tokens, i, cx);
      if (found && tokens.slice(i, i + found[0]).some((x) => x.masked)) found = null;
    }
    if (!found) {
      i++;
      continue;
    }
    const [used, meanings] = found;
    const from = (tokens[i] as Token).from;
    const to = (tokens[i + used - 1] as Token).to;
    const word = text.slice(from, to);
    for (const meaning of meanings) {
      const tally = `${meaning.kind}:${word.toLowerCase()}`;
      const n = (seen.get(tally) ?? 0) + 1;
      seen.set(tally, n);
      out.push({ start: from, end: to, word, meaning, key: `${tally}:${n}` });
    }
    i += used;
  }
  return out;
}

/** Reads the text. `lists` are the Lists `#Name` may match, in their order; `fold` ignores case and accents. */
export function parseText(
  req: ParseRequest,
  lists: ListRef[],
  fold: (s: string) => string,
): Parsed {
  const cx: Context = {
    today: req.today,
    weekStart: req.week_start === 'sun' ? 0 : 1,
    lists,
    fold,
  };
  const frozen = req.frozen ?? [];
  const ignoredKeys = req.ignored ?? [];
  const prefer = req.prefer ?? {};
  const cands = candidates(req.text, cx).filter(
    (c) => !frozen.some((r) => c.start >= r.start && c.end <= r.end),
  );
  const ignored = cands.filter((c) => ignoredKeys.includes(c.key));
  const live = cands.filter((c) => !ignoredKeys.includes(c.key));

  const won: Cand[] = [];
  const lost: Cand[] = [];
  // One winner per kind: the user's pick, else the last; earlier ones are overridden.
  for (const kind of KINDS) {
    const ofKind = live.filter((c) => c.meaning.kind === kind);
    const winner = ofKind.find((c) => c.key === prefer[kind]) ?? ofKind[ofKind.length - 1];
    if (!winner) continue;
    won.push(winner);
    lost.push(...ofKind.filter((c) => c.key !== winner.key));
  }

  const out: Parsed = {
    spans: [],
    alternatives: [],
    ignored: [],
    date: null,
    time: null,
    repeat: null,
    priority: null,
    list_id: null,
    new_list: null,
  };
  for (const c of won) {
    const m = c.meaning;
    if (m.kind === 'date') out.date = m.value;
    else if (m.kind === 'time') out.time = m.value;
    else if (m.kind === 'repeat') out.repeat = m.rule;
    else if (m.kind === 'prio') out.priority = m.value;
    else if (m.id !== null) out.list_id = m.id;
    else out.new_list = m.name;
  }
  // A time alone means today if it is still ahead, otherwise tomorrow. With a Repeat the Task
  // lands on the Repeat's first day instead, which the command works out.
  if (out.time !== null && out.date === null && out.repeat === null) {
    out.date = out.time >= req.now ? req.today : addDays(req.today, 1);
  }
  const byPlace = (a: Cand, b: Cand) => a.start - b.start || a.end - b.end;
  out.spans = won.sort(byPlace).map(toSpan);
  out.alternatives = lost
    .sort(byPlace)
    .map((c): ParseAlternative => ({ ...toSpan(c), reason: 'conflict' }));
  out.ignored = ignored.map(toSpan);
  return out;
}
