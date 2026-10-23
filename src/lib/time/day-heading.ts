import { daysBetween, dayText } from '../ui/when-grid';

const WEEKDAYS = ['Sunday', 'Monday', 'Tuesday', 'Wednesday', 'Thursday', 'Friday', 'Saturday'];

/** A serif day heading: the name in the display font and the small date beside it. */
export interface DayHeading {
  /** "Today", "Yesterday" or "Friday". */
  name: string;
  /** "23 Oct", with the year when it is not this year. */
  small: string;
}

/**
 * A heading of Done and of Activity: `Today`, `Yesterday`, then the weekday, with the small date
 * beside it. Both dates are `YYYY-MM-DD` on the device's calendar.
 */
export function doneHeading(date: string, today: string): DayHeading {
  const n = daysBetween(date, today);
  const small = dayText(date, today).split(' ').slice(1).join(' ');
  if (n === 0) return { name: 'Today', small };
  if (n === 1) return { name: 'Yesterday', small };
  return { name: WEEKDAYS[new Date(`${date}T00:00:00Z`).getUTCDay()] ?? '', small };
}
