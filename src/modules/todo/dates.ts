import type { LocalNow, Task } from '../../lib/ipc/bindings';
import type { TimeFormat } from '../../lib/time/format';
import { daysBetween, dayText, monthShort, weekdayShort } from '../../lib/ui/when-grid';
import { type RepeatRule, repeatLabel } from '../../lib/ui/when-repeat';
import { formatHm } from '../../lib/ui/when-time';
import { quoted } from './tasks';

const pad = (n: number) => String(n).padStart(2, '0');

const WEEKDAYS = ['Sunday', 'Monday', 'Tuesday', 'Wednesday', 'Thursday', 'Friday', 'Saturday'];
const MONTHS = [
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

/** `YYYY-MM-DD` of `now` on the device's calendar. */
export const localDate = (now: Date): string =>
  `${now.getFullYear()}-${pad(now.getMonth() + 1)}-${pad(now.getDate())}`;

/** `HH:MM` of `now` on the device's clock. */
export const localTime = (now: Date): string => `${pad(now.getHours())}:${pad(now.getMinutes())}`;

/** What every date-dependent command is told: the device's today, now and offset from UTC. */
export const localNow = (now: Date): LocalNow => ({
  today: localDate(now),
  now: localTime(now),
  offset_min: -now.getTimezoneOffset(),
});

/**
 * A row's date: Today, Tomorrow, Yesterday, a weekday within the week ahead, then "28 Oct" with the
 * year when it is not this year.
 */
export function rowDateLabel(date: string, today: string): string {
  const n = daysBetween(today, date);
  if (n === 0) return 'Today';
  if (n === 1) return 'Tomorrow';
  if (n === -1) return 'Yesterday';
  if (n > 1 && n < 7) return weekdayShort(date);
  const [y = 0, , d = 1] = date.split('-').map(Number);
  const year = y === Number(today.slice(0, 4)) ? '' : ` ${y}`;
  return `${d} ${monthShort(date)}${year}`;
}

/** An open Task whose date is before today. A Task due today is never Overdue, whatever its time. */
export const isOverdue = (task: Pick<Task, 'state' | 'due_date'>, today: string): boolean =>
  task.state === 'open' && task.due_date !== null && task.due_date < today;

/** A Task due today whose time is behind us: it stays in Today with its time in red. */
export const timePassed = (
  task: Pick<Task, 'state' | 'due_date' | 'due_time'>,
  today: string,
  now: string,
): boolean =>
  task.state === 'open' && task.due_date === today && task.due_time !== null && task.due_time < now;

/** "Wednesday, 21 October". */
export function longDate(date: string): string {
  const [, m = 1, d = 1] = date.split('-').map(Number);
  const weekday = WEEKDAYS[new Date(`${date}T00:00:00Z`).getUTCDay()];
  return `${weekday}, ${d} ${MONTHS[m - 1]}`;
}

export interface DayHeading {
  /** "Tomorrow" or "Friday". */
  name: string;
  /** "23 Oct", with the year when it is not this year. */
  small: string;
}

/** The serif header of an Upcoming day. */
export function dayHeading(date: string, today: string): DayHeading {
  const weekday = WEEKDAYS[new Date(`${date}T00:00:00Z`).getUTCDay()] ?? '';
  const small = dayText(date, today).split(' ').slice(1).join(' ');
  return { name: daysBetween(today, date) === 1 ? 'Tomorrow' : weekday, small };
}

/** The wall-clock time a Task was closed, in the device's zone, as the Setting wants it. */
export function closedTime(closedAt: number, format: TimeFormat): string {
  return formatHm(localTime(new Date(closedAt)), format);
}

/** A Task's Repeat, or none. */
export function taskRepeat(task: Task): RepeatRule | null {
  if (task.repeat_freq === null || task.repeat_every === null || task.repeat_mode === null) {
    return null;
  }
  return {
    freq: task.repeat_freq,
    every: task.repeat_every,
    days: task.repeat_days,
    mode: task.repeat_mode,
  };
}

/**
 * The Undo toast after a date change, e.g. `“Pay rent” → Tomorrow · 09:00 · Every month` or
 * `→ No date`. A Repeat that was there and is gone says `No repeat`.
 */
export function whenMessage(
  title: string,
  date: string | null,
  time: string | null,
  today: string,
  format: TimeFormat,
  repeat: RepeatRule | null = null,
  hadRepeat = false,
): string {
  const day = date === null ? null : rowDateLabel(date, today);
  const at = time === null ? null : formatHm(time, format);
  const rule = repeat ? repeatLabel(repeat) : date !== null && hadRepeat ? 'No repeat' : null;
  const when = [day, at, rule].filter((p) => p !== null).join(' · ');
  return `${quoted(title)} → ${when || 'No date'}`;
}

/** "Sat 24 Oct": a date named in a toast or menu, with the year when it is not this year's. */
export const nextText = (date: string, today: string): string => dayText(date, today);

/** The toast after ticking a repeating Task: `Completed “Water the plants” · next Sat 24 Oct`. */
export const completedRepeatMessage = (title: string, next: string, today: string): string =>
  `Completed ${quoted(title)} · next ${nextText(next, today)}`;

/** The toast after skipping: `Skipped “Water the plants” · next Sat 24 Oct`. */
export const skippedMessage = (title: string, next: string, today: string): string =>
  `Skipped ${quoted(title)} · next ${nextText(next, today)}`;

/** A heading of Done: `Today`, `Yesterday`, then the weekday, with the small date beside it. */
export function doneHeading(date: string, today: string): DayHeading {
  const n = daysBetween(date, today);
  const small = dayText(date, today).split(' ').slice(1).join(' ');
  if (n === 0) return { name: 'Today', small };
  if (n === 1) return { name: 'Yesterday', small };
  return { name: WEEKDAYS[new Date(`${date}T00:00:00Z`).getUTCDay()] ?? '', small };
}

/** The toast after Move all: `Moved 3 Tasks to Today`. */
export function movedAllMessage(count: number, date: string, today: string): string {
  const where = date === today ? 'Today' : rowDateLabel(date, today);
  return `Moved ${count} ${count === 1 ? 'Task' : 'Tasks'} to ${where}`;
}
