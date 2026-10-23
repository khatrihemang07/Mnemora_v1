/**
 * Words for "Resolved automatically" in the browser mock, as crates/sync/src/resolved_view.rs
 * writes them: dates and times the way the person's Preferences say (`core.date_format`,
 * `core.time_format`), a Task's title in quotes, a value as a person reads it.
 */
import type { Json } from './mock-sync-merge';

export type DateStyle = 'long' | 'short' | 'iso';
export type TimeStyle = '24h' | '12h';
export interface Formats {
  date: DateStyle;
  time: TimeStyle;
}
export const DEFAULT_FORMATS: Formats = { date: 'long', time: '24h' };

const DAYS = ['Sun', 'Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat'];
const MONTHS = ['Jan', 'Feb', 'Mar', 'Apr', 'May', 'Jun', 'Jul', 'Aug', 'Sep', 'Oct', 'Nov', 'Dec'];

const pad = (n: number, width = 2) => String(n).padStart(width, '0');

function dateWords(y: number, m: number, d: number, style: DateStyle): string {
  const weekday = DAYS[new Date(Date.UTC(y, m - 1, d)).getUTCDay()];
  const month = MONTHS[m - 1];
  if (style === 'short') return `${month} ${d}, ${y}`;
  if (style === 'iso') return `${pad(y, 4)}-${pad(m)}-${pad(d)}`;
  return `${weekday} ${d} ${month}`;
}

/** A `YYYY-MM-DD` date written the way the person chose (anything else is shown as it is). */
export function dateText(date: string, style: DateStyle): string {
  const [y, m, d] = date.split('-').map(Number);
  if (!y || !m || !d || m > 12 || d > 31) return date;
  return dateWords(y, m, d, style);
}

/** A time of day, from the minutes since midnight. */
export function clockWords(minuteOfDay: number, style: TimeStyle): string {
  const h = Math.floor(minuteOfDay / 60);
  const m = minuteOfDay % 60;
  if (style === '24h') return `${pad(h)}:${pad(m)}`;
  return `${h % 12 === 0 ? 12 : h % 12}:${pad(m)} ${h < 12 ? 'AM' : 'PM'}`;
}

/** An `HH:MM` time written the way the person chose. */
export function timeText(time: string, style: TimeStyle): string {
  const [h, m] = time.split(':').map(Number);
  if (h === undefined || m === undefined || Number.isNaN(h + m) || h > 23 || m > 59) return time;
  return clockWords(h * 60 + m, style);
}

const text = (v: Json | undefined): string | null => (typeof v === 'string' && v !== '' ? v : null);

/** "daily", "weekly", "every 2 weeks". */
function repeatWords(values: Record<string, Json>): string | null {
  const freq = text(values.repeat_freq);
  const every = typeof values.repeat_every === 'number' ? values.repeat_every : 1;
  const words: Record<string, [string, string]> = {
    day: ['daily', 'days'],
    week: ['weekly', 'weeks'],
    month: ['monthly', 'months'],
    year: ['yearly', 'years'],
  };
  const w = freq ? words[freq] : undefined;
  if (!w) return null;
  return every <= 1 ? w[0] : `every ${every} ${w[1]}`;
}

/** What a unit is called ("Due date"), as `unit_words` does. */
export const UNIT_WORDS: Record<string, string> = {
  priority: 'Priority',
  when: 'Due date',
  state: 'Status',
  list_id: 'List',
  deleted_at: 'Deleted',
  name: 'Name',
  emoji: 'Emoji',
  task_sort: 'Sort',
  group_by: 'Grouping',
  page_date: 'Page',
  written: 'Time written',
  task_id: 'Task',
  title: 'Title',
  notes: 'Notes',
  body: 'Text',
};

const SORT_WORDS: Record<string, string> = {
  manual: 'Manual',
  due: 'Due date',
  priority: 'Priority',
  title: 'Title',
  added: 'Date added',
  none: 'None',
  list: 'List',
  day: 'Day',
};

/** How a value reads, as `value_words` does. `listName` finds a List's name by its id. */
export function valueWords(
  entity: string,
  unit: string,
  values: Record<string, Json>,
  f: Formats,
  listName: (id: string) => string,
): string {
  if (unit === 'deleted_at') return values.deleted_at != null ? 'Deleted' : 'Restored';
  if (entity === 'task') {
    if (unit === 'priority') return `P${typeof values.priority === 'number' ? values.priority : 4}`;
    if (unit === 'when') {
      const parts = [
        text(values.due_date) ? dateText(text(values.due_date) as string, f.date) : 'No date',
      ];
      const time = text(values.due_time);
      if (time) parts.push(timeText(time, f.time));
      const repeat = repeatWords(values);
      return repeat ? `${parts.join(' ')} · repeats ${repeat}` : parts.join(' ');
    }
    if (unit === 'state')
      return values.state === 'done' ? 'Done' : values.state === 'wont' ? 'Won’t do' : 'Open';
    if (unit === 'list_id') {
      const id = text(values.list_id);
      return id ? listName(id) : 'Inbox';
    }
  }
  if (entity === 'list') {
    if (unit === 'name') return text(values.name) ?? 'Untitled';
    if (unit === 'emoji') return text(values.emoji) ?? 'No emoji';
    if (unit === 'task_sort' || unit === 'group_by') {
      const v = text(values[unit]);
      return v ? (SORT_WORDS[v] ?? v) : '';
    }
  }
  if (entity === 'entry' && unit === 'page_date') {
    const d = text(values.page_date);
    return d ? dateText(d, f.date) : '';
  }
  return Object.values(values)
    .map((v) => (typeof v === 'string' ? v : JSON.stringify(v)))
    .join(' ');
}
