import { describe, expect, it } from 'vitest';
import type { AllView, InboxView, Task, TodayView, UpcomingView } from '../../lib/ipc/bindings';
import {
  completedMessage,
  quoted,
  withoutTask,
  withoutTaskInAll,
  withoutTaskInToday,
  withoutTaskInUpcoming,
} from './tasks';

const task = (id: string): Task => ({ id, title: id }) as Task;

describe('completedMessage', () => {
  it('names the Task in curly quotes', () => {
    expect(completedMessage('Buy oat milk')).toBe('Completed “Buy oat milk”');
  });

  it('cuts a long title at 34 characters with an ellipsis', () => {
    const long = 'a'.repeat(40);
    expect(completedMessage(long)).toBe(`Completed “${'a'.repeat(34)}…”`);
    expect(completedMessage('b'.repeat(34))).toBe(`Completed “${'b'.repeat(34)}”`);
  });

  it('cuts by characters, not UTF-16 units, and drops a trailing space', () => {
    expect(completedMessage(`${'😀'.repeat(33)} xyz`)).toBe(`Completed “${'😀'.repeat(33)}…”`);
  });
});

describe('withoutTask', () => {
  it('drops only that Task and keeps the order', () => {
    const view: InboxView = { tasks: [task('a'), task('b'), task('c')] };
    expect(withoutTask(view, 'b').tasks.map((t) => t.id)).toEqual(['a', 'c']);
    expect(view.tasks).toHaveLength(3);
  });
});

describe('quoted', () => {
  it('cuts a long name the same way', () => {
    expect(quoted('Home')).toBe('“Home”');
    expect(quoted('x'.repeat(40))).toBe(`“${'x'.repeat(34)}…”`);
  });
});

describe('withoutTaskInAll', () => {
  it('drops the Task and a group it leaves empty, keeping the order', () => {
    const view: AllView = {
      groups: [
        { list: null, tasks: [task('a')] },
        { list: { id: 'l1' } as never, tasks: [task('b'), task('c')] },
      ],
    };
    expect(withoutTaskInAll(view, 'b').groups.map((g) => g.tasks.map((t) => t.id))).toEqual([
      ['a'],
      ['c'],
    ]);
    expect(withoutTaskInAll(view, 'a').groups.map((g) => g.list?.id)).toEqual(['l1']);
    expect(view.groups).toHaveLength(2);
  });
});

describe('withoutTaskInToday', () => {
  it('drops the Task from Overdue or the day and leaves the rest alone', () => {
    const view: TodayView = {
      overdue: [task('a'), task('b')],
      tasks: [task('c')],
      done: [task('d')],
      total: 4,
    };
    expect(withoutTaskInToday(view, 'a').overdue.map((t) => t.id)).toEqual(['b']);
    const without = withoutTaskInToday(view, 'c');
    expect(without.tasks).toEqual([]);
    expect(without.done).toHaveLength(1);
    expect(view.tasks).toHaveLength(1);
  });
});

describe('withoutTaskInUpcoming', () => {
  it('drops the Task and a day it leaves empty, keeping the order', () => {
    const view: UpcomingView = {
      overdue: [task('a')],
      days: [
        { date: '2026-10-22', tasks: [task('b')] },
        { date: '2026-10-23', tasks: [task('c'), task('d')] },
      ],
    };
    expect(withoutTaskInUpcoming(view, 'b').days.map((d) => d.date)).toEqual(['2026-10-23']);
    expect(withoutTaskInUpcoming(view, 'c').days[1]?.tasks.map((t) => t.id)).toEqual(['d']);
    expect(withoutTaskInUpcoming(view, 'a').overdue).toEqual([]);
    expect(view.days).toHaveLength(2);
  });
});
