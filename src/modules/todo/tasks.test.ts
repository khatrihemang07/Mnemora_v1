import { describe, expect, it } from 'vitest';
import type { Task, TaskGroup } from '../../lib/ipc/bindings';
import {
  completedMessage,
  deletedMessage,
  priorityMessage,
  priorityTone,
  quoted,
  withoutTaskInGroups,
  wontMessage,
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

describe('quoted', () => {
  it('cuts a long name the same way', () => {
    expect(quoted('Home')).toBe('“Home”');
    expect(quoted('x'.repeat(40))).toBe(`“${'x'.repeat(34)}…”`);
  });
});

describe('withoutTaskInGroups', () => {
  const group = (kind: 'plain' | 'overdue', ...ids: string[]) =>
    ({ key: { kind }, adopt: [], tasks: ids.map(task) }) as TaskGroup;
  const ids = (view: { groups: TaskGroup[] }) => view.groups.map((g) => g.tasks.map((t) => t.id));

  it('drops only that Task and keeps the order', () => {
    const view = { groups: [group('plain', 'a', 'b', 'c')] };
    expect(ids(withoutTaskInGroups(view, 'b'))).toEqual([['a', 'c']]);
    expect(view.groups[0]?.tasks).toHaveLength(3);
  });

  it('drops a group it leaves empty, but never the plain group', () => {
    const view = { groups: [group('overdue', 'a'), group('plain', 'b', 'c')] };
    expect(ids(withoutTaskInGroups(view, 'a'))).toEqual([['b', 'c']]);
    const only = { groups: [group('plain', 'a')] };
    expect(ids(withoutTaskInGroups(only, 'a'))).toEqual([[]]);
  });

  it('leaves the rest of the view alone', () => {
    const view = { groups: [group('plain', 'a')], done: [task('d')], total: 2 };
    expect(withoutTaskInGroups(view, 'a').done).toHaveLength(1);
    expect(withoutTaskInGroups(view, 'a').total).toBe(2);
  });
});

describe('toast wording', () => {
  it('shows a title without its Markdown marks', () => {
    expect(completedMessage('Pay **rent** at [the bank](https://x.y)')).toBe(
      'Completed “Pay rent at the bank”',
    );
  });

  it('names the Task for Won’t do, delete and Priority', () => {
    expect(wontMessage('Renew gym card')).toBe('Won’t do “Renew gym card”');
    expect(deletedMessage('Renew gym card')).toBe('Deleted “Renew gym card”');
    expect(priorityMessage('Pay rent', 1)).toBe('“Pay rent” → P1');
    expect(priorityMessage('Pay rent', 4)).toBe('“Pay rent” → No priority');
  });

  it('colours the ring for P1 to P3 only', () => {
    expect([1, 2, 3, 4].map(priorityTone)).toEqual(['p1', 'p2', 'p3', 'plain']);
  });
});
