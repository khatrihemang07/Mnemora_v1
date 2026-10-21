import { describe, expect, it } from 'vitest';
import {
  closedTime,
  dayHeading,
  isOverdue,
  landedIn,
  localNow,
  longDate,
  movedAllMessage,
  rowDateLabel,
  stampLabel,
  timePassed,
  whenChipLabel,
  whenMessage,
} from './dates';

// 21 October 2026 is a Wednesday.
const TODAY = '2026-10-21';

describe('rowDateLabel', () => {
  it('uses Today, Tomorrow and Yesterday', () => {
    expect(rowDateLabel('2026-10-21', TODAY)).toBe('Today');
    expect(rowDateLabel('2026-10-22', TODAY)).toBe('Tomorrow');
    expect(rowDateLabel('2026-10-20', TODAY)).toBe('Yesterday');
  });

  it('names the weekday for the rest of the coming week', () => {
    expect(rowDateLabel('2026-10-23', TODAY)).toBe('Fri');
    expect(rowDateLabel('2026-10-27', TODAY)).toBe('Tue');
  });

  it('switches to the day and month from a week ahead, and for older dates', () => {
    expect(rowDateLabel('2026-10-28', TODAY)).toBe('28 Oct');
    expect(rowDateLabel('2026-10-19', TODAY)).toBe('19 Oct');
    expect(rowDateLabel('2026-11-03', TODAY)).toBe('3 Nov');
  });

  it('adds the year when it is not this year', () => {
    expect(rowDateLabel('2027-01-05', TODAY)).toBe('5 Jan 2027');
    expect(rowDateLabel('2025-12-31', TODAY)).toBe('31 Dec 2025');
  });
});

describe('overdue and passed times', () => {
  it('is Overdue by date only', () => {
    expect(isOverdue({ state: 'open', due_date: '2026-10-20' }, TODAY)).toBe(true);
    expect(isOverdue({ state: 'open', due_date: TODAY }, TODAY)).toBe(false);
    expect(isOverdue({ state: 'open', due_date: null }, TODAY)).toBe(false);
    expect(isOverdue({ state: 'done', due_date: '2026-10-20' }, TODAY)).toBe(false);
  });

  it('marks a time red once it is behind us on its day', () => {
    const task = { state: 'open', due_date: TODAY, due_time: '09:00' } as const;
    expect(timePassed(task, TODAY, '09:01')).toBe(true);
    expect(timePassed(task, TODAY, '09:00')).toBe(false);
    expect(timePassed(task, TODAY, '08:00')).toBe(false);
    expect(timePassed({ ...task, due_date: '2026-10-22' }, TODAY, '23:00')).toBe(false);
    expect(timePassed({ ...task, state: 'done' }, TODAY, '23:00')).toBe(false);
    expect(timePassed({ ...task, due_time: null }, TODAY, '23:00')).toBe(false);
  });
});

describe('headings', () => {
  it('writes the full date', () => {
    expect(longDate(TODAY)).toBe('Wednesday, 21 October');
    expect(longDate('2026-01-01')).toBe('Thursday, 1 January');
  });

  it('heads an Upcoming day with its name and a small date', () => {
    expect(dayHeading('2026-10-22', TODAY)).toEqual({ name: 'Tomorrow', small: '22 Oct' });
    expect(dayHeading('2026-10-23', TODAY)).toEqual({ name: 'Friday', small: '23 Oct' });
    expect(dayHeading('2027-01-05', TODAY)).toEqual({ name: 'Tuesday', small: '5 Jan 2027' });
  });
});

describe('localNow', () => {
  it('reads the device calendar, clock and offset east of UTC', () => {
    const at = new Date(2026, 9, 21, 14, 5, 30);
    expect(localNow(at)).toEqual({
      today: '2026-10-21',
      now: '14:05',
      offset_min: -at.getTimezoneOffset(),
    });
  });

  it('rolls over at local midnight', () => {
    expect(localNow(new Date(2026, 9, 21, 23, 59, 59)).today).toBe('2026-10-21');
    expect(localNow(new Date(2026, 9, 22, 0, 0, 0)).today).toBe('2026-10-22');
  });
});

describe('messages', () => {
  it('names the Task and where it went', () => {
    expect(whenMessage('Pay rent', '2026-10-22', '09:00', TODAY, '24h')).toBe(
      '“Pay rent” → Tomorrow · 09:00',
    );
    expect(whenMessage('Pay rent', '2026-10-21', null, TODAY, '24h')).toBe('“Pay rent” → Today');
    expect(whenMessage('Pay rent', null, null, TODAY, '24h')).toBe('“Pay rent” → No date');
    expect(whenMessage('Pay rent', '2026-10-23', '21:30', TODAY, '12h')).toBe(
      '“Pay rent” → Fri · 9:30 PM',
    );
  });

  it('counts the Tasks Move all moved', () => {
    expect(movedAllMessage(3, TODAY, TODAY)).toBe('Moved 3 Tasks to Today');
    expect(movedAllMessage(1, '2026-10-25', TODAY)).toBe('Moved 1 Task to Sun');
  });

  it('shows the time a Task was closed on the device clock', () => {
    const at = new Date(2026, 9, 21, 9, 12).getTime();
    expect(closedTime(at, '24h')).toBe('09:12');
    expect(closedTime(at, '12h')).toBe('9:12 AM');
  });
});

describe('stampLabel', () => {
  // Built from local parts, so the test holds in any time zone.
  const at = (y: number, m: number, d: number, h = 9, min = 12) =>
    new Date(y, m - 1, d, h, min).getTime();

  it('says Today and Yesterday with the time', () => {
    expect(stampLabel(at(2026, 10, 21), TODAY, '24h')).toBe('Today 09:12');
    expect(stampLabel(at(2026, 10, 20, 18, 40), TODAY, '24h')).toBe('Yesterday 18:40');
    expect(stampLabel(at(2026, 10, 21, 18, 40), TODAY, '12h')).toBe('Today 6:40 PM');
  });

  it('names a weekday inside the last week, then the date', () => {
    expect(stampLabel(at(2026, 10, 19, 8, 0), TODAY, '24h')).toBe('Mon 08:00');
    expect(stampLabel(at(2026, 10, 14, 8, 0), TODAY, '24h')).toBe('14 Oct 08:00');
    expect(stampLabel(at(2025, 12, 31, 8, 0), TODAY, '24h')).toBe('31 Dec 2025 08:00');
  });

  it('can leave the time out', () => {
    expect(stampLabel(at(2026, 10, 14), TODAY, '24h', false)).toBe('14 Oct');
  });
});

describe('whenChipLabel', () => {
  const none = {
    due_date: null,
    due_time: null,
    repeat_freq: null,
    repeat_every: null,
    repeat_days: null,
    repeat_mode: null,
  };

  it('is When while the Task has no date', () => {
    expect(whenChipLabel(none, TODAY, '24h')).toBe('When');
  });

  it('joins the day, the time and the Repeat', () => {
    expect(
      whenChipLabel({ ...none, due_date: '2026-10-22', due_time: '09:00' }, TODAY, '24h'),
    ).toBe('Tomorrow · 09:00');
    const repeating = {
      ...none,
      due_date: '2026-10-21',
      repeat_freq: 'month',
      repeat_every: 1,
      repeat_days: 21,
      repeat_mode: 'fixed',
    };
    expect(whenChipLabel(repeating, TODAY, '24h')).toBe('Today · Every month');
  });
});

describe('landedIn', () => {
  it('names the day for a dated Task and the List or Inbox for an undated one', () => {
    expect(landedIn({ due_date: '2026-10-22' }, null, TODAY)).toBe('Upcoming · Thu 22 Oct');
    expect(landedIn({ due_date: '2026-10-21' }, null, TODAY)).toBe('Today');
    expect(landedIn({ due_date: '2026-10-20' }, null, TODAY)).toBe('Today');
    expect(landedIn({ due_date: null }, 'Groceries', TODAY)).toBe('Groceries');
    expect(landedIn({ due_date: null }, null, TODAY)).toBe('Inbox');
  });
});
