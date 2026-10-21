import { describe, expect, it } from 'vitest';
import type { InboxView, Task } from '../../lib/ipc/bindings';
import { completedMessage, withoutTask } from './tasks';

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
