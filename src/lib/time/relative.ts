import { dayText } from '../ui/when-grid';

const MIN = 60_000;
const HOUR = 60 * MIN;
const DAY = 24 * HOUR;

/** N15-A: relative under 7 days, absolute after (callers show the absolute time on hover). */
export function relativeTime(date: Date, now: Date = new Date()): string {
  const diff = now.getTime() - date.getTime();
  if (diff < MIN) return 'just now';
  if (diff < HOUR) return `${Math.floor(diff / MIN)} min ago`;
  if (diff < DAY) return `${Math.floor(diff / HOUR)} h ago`;
  const days = Math.floor(diff / DAY);
  if (days === 1) return 'yesterday';
  if (days < 7) return `${days} days ago`;
  return absoluteDay(date, now);
}

const ymd = (d: Date) =>
  `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}-${String(d.getDate()).padStart(2, '0')}`;

/** "Wed 14 Oct", with the year when it is not `now`'s year: the app's style for a date named in a sentence. */
export function absoluteDay(date: Date, now: Date = new Date()): string {
  return dayText(ymd(date), ymd(now));
}
