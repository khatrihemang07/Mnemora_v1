import type { LocalContext } from '../../lib/ipc/bindings';

/** The device's calendar, as a question sends it: the date, weekday, time zone and week start. */
export function localContext(now: Date, weekStart: 'mon' | 'sun', timeZone: string): LocalContext {
  const pad = (n: number) => String(n).padStart(2, '0');
  return {
    today: `${now.getFullYear()}-${pad(now.getMonth() + 1)}-${pad(now.getDate())}`,
    weekday: now.toLocaleDateString('en-US', { weekday: 'long' }),
    time_zone: timeZone,
    week_start: weekStart,
  };
}
