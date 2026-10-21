import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { Task } from '../../lib/ipc/bindings';

// The actions talk to IPC, toasts and runes stores; stand them in so the order of calls is what
// is tested (ADR 0006: no DOM, so the Undo callback is read out of the toast).
const undos: Array<() => void> = [];
const calls: string[] = [];
let reopened: Partial<Task> = {};

vi.mock('./queries', () => ({
  completeTask: vi.fn(async (id: string) => ({ task: { id }, occurrence: null })),
  reopenTask: vi.fn(async (id: string) => {
    calls.push(`reopen ${id}`);
    return reopened;
  }),
  updateTask: vi.fn(async (id: string, title: string, notes: string) => {
    calls.push(`update ${id} ${title} ${JSON.stringify(notes)}`);
    return { id, title, notes };
  }),
  invalidateTodo: vi.fn(async () => {}),
  removeCachedTask: vi.fn(),
  deleteTask: vi.fn(),
  moveOverdue: vi.fn(),
  moveTask: vi.fn(),
  reorderTask: vi.fn(),
  restoreTask: vi.fn(),
  restoreWhen: vi.fn(),
  setPriority: vi.fn(),
  setWhen: vi.fn(),
  skipTask: vi.fn(),
  wontDo: vi.fn(),
}));
vi.mock('../../lib/ui/toast', () => ({
  notifyError: vi.fn((m: string) => calls.push(`error ${m}`)),
  notifyUndo: vi.fn((_m: string, undo: () => void) => undos.push(undo)),
}));
vi.mock('../../lib/viewport.svelte', () => ({ viewport: { isPhone: false } }));
vi.mock('../../lib/haptic', () => ({ lightTick: vi.fn() }));
vi.mock('./clock.svelte', () => ({ todoClock: { today: '2026-10-21' } }));
vi.mock('./prefs.svelte', () => ({ todoPrefs: {} }));
vi.mock('./steps-prompt.svelte', () => ({ stepsPrompt: { ask: vi.fn() } }));

import { tickAllAndComplete } from './task-actions';

const task = (notes: string): Task =>
  ({ id: 't1', title: 'Pack', notes, state: 'open', repeat_freq: null }) as Task;

beforeEach(() => {
  undos.length = 0;
  calls.length = 0;
});

describe('Undo after "Tick all and complete"', () => {
  it('reopens the Task and puts the original notes back', async () => {
    const original = '- [x] tent\n- [ ] stove';
    reopened = { id: 't1', title: 'Pack', notes: '- [x] tent\n- [x] stove' };
    await tickAllAndComplete(task(original));
    expect(calls).toEqual(['update t1 Pack "- [x] tent\\n- [x] stove"']);
    calls.length = 0;
    undos[0]?.();
    await vi.waitFor(() =>
      expect(calls).toEqual(['reopen t1', `update t1 Pack ${JSON.stringify(original)}`]),
    );
  });
});
