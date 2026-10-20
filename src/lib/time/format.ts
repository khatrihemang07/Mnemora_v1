export type TimeFormat = '24h' | '12h';
export type DateFormat = 'long' | 'short' | 'iso';

const LOCALE = 'en-US';
const pad = (n: number) => String(n).padStart(2, '0');

export interface ClockParts {
  hm: string;
  seconds: string;
  period: 'AM' | 'PM' | null;
}

/** Split so the UI can render seconds smaller than hours:minutes. */
export function clockParts(date: Date, format: TimeFormat): ClockParts {
  const h = date.getHours();
  const seconds = pad(date.getSeconds());
  const minutes = pad(date.getMinutes());
  if (format === '24h') return { hm: `${pad(h)}:${minutes}`, seconds, period: null };
  return { hm: `${h % 12 || 12}:${minutes}`, seconds, period: h < 12 ? 'AM' : 'PM' };
}

export function formatDate(date: Date, format: DateFormat): string {
  if (format === 'iso') {
    return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())}`;
  }
  const opts: Intl.DateTimeFormatOptions =
    format === 'long'
      ? { weekday: 'long', month: 'long', day: 'numeric' }
      : { month: 'short', day: 'numeric', year: 'numeric' };
  return new Intl.DateTimeFormat(LOCALE, opts).format(date);
}
