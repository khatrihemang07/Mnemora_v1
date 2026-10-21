/** Typing a time of day, and showing one. Times are `HH:MM` on a 24-hour clock and have no zone. */

import type { TimeFormat } from '../time/format';

const pad = (n: number) => String(n).padStart(2, '0');

/**
 * What a typed time can mean, best guess first. "9" is 09:00 or 21:00, "930p" is 21:30, "21:30"
 * is itself, "noon" and "midnight" are words. Nothing typed, or something that is not a time,
 * gives nothing: the panel shows matches only as you type.
 */
export function timeSuggestions(input: string): string[] {
  const s = input.trim().toLowerCase().replace(/\s+/g, '');
  if (!s) return [];
  if (s === 'noon') return ['12:00'];
  if (s === 'midnight') return ['00:00'];
  const m = s.match(/^(\d{1,2})(?::?(\d{2}))?(a|am|p|pm)?$/);
  if (!m) return [];
  const hour = Number(m[1]);
  const minute = m[2] ? Number(m[2]) : 0;
  const mark = m[3];
  if (hour > 23 || minute > 59) return [];
  const hm = (h: number) => `${pad(h)}:${pad(minute)}`;
  if (mark) {
    if (hour > 12 || hour === 0) return [];
    const pm = mark.startsWith('p');
    return [hm(pm ? (hour === 12 ? 12 : hour + 12) : hour === 12 ? 0 : hour)];
  }
  // A bare hour could be either half of the day; "13" and "21" can only be one.
  if (hour >= 1 && hour <= 12) return [...new Set([hm(hour), hm(hour === 12 ? 0 : hour + 12)])];
  return [hm(hour)];
}

/** "21:30" as the user's Setting wants it: "21:30" or "9:30 PM". */
export function formatHm(hm: string, format: TimeFormat): string {
  if (format === '24h') return hm;
  const [h = 0, m = 0] = hm.split(':').map(Number);
  return `${h % 12 || 12}:${pad(m)} ${h < 12 ? 'AM' : 'PM'}`;
}
