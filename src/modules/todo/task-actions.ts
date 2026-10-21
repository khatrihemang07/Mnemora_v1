import type { Completion, DueChange, Task, TodoList } from '../../lib/ipc/bindings';
import { IpcError } from '../../lib/ipc/unwrap';
import { notifyError, notifyUndo } from '../../lib/ui/toast';
import type { RepeatRule } from '../../lib/ui/when-repeat';
import { todoClock } from './clock.svelte';
import {
  completedRepeatMessage,
  movedAllMessage,
  skippedMessage,
  taskRepeat,
  whenMessage,
} from './dates';
import { movedMessage } from './lists';
import { todoPrefs } from './prefs.svelte';
import {
  completeTask,
  invalidateTodo,
  moveOverdue,
  moveTask,
  removeCachedTask,
  reopenTask,
  restoreWhen,
  setWhen,
  skipTask,
} from './queries';
import { completedMessage } from './tasks';

/**
 * Ticks a Task: it leaves the page at once, then "Completed “…”" offers Undo, which reopens it in
 * its old place. A repeating Task stays: the toast says where it went ("· next Sat 24 Oct") and
 * Undo reopens that occurrence, which rewinds the Task. A failed write brings the Task back from
 * Rust's truth.
 */
export async function tickTask(task: Task): Promise<void> {
  const repeating = task.repeat_freq !== null;
  if (!repeating) removeCachedTask(task.id);
  let done: Completion;
  try {
    done = await completeTask(task.id);
  } catch {
    void invalidateTodo();
    notifyError("Couldn't complete the Task");
    return;
  }
  void invalidateTodo();
  const next = done.occurrence && done.task.due_date;
  const message = next
    ? completedRepeatMessage(task.title, next, todoClock.today)
    : completedMessage(task.title);
  notifyUndo(message, () => {
    reopenTask(done.occurrence?.id ?? task.id)
      .catch(() => notifyError("Couldn't reopen the Task"))
      .finally(() => void invalidateTodo());
  });
}

/** Moves a Task to a List (null is the Inbox); Undo puts it back where it was. */
export async function moveTaskTo(task: Task, to: TodoList | null): Promise<void> {
  const from = task.list_id;
  if ((to?.id ?? null) === from) return;
  try {
    await moveTask(task.id, to?.id ?? null);
  } catch {
    notifyError("Couldn't move the Task");
    return;
  } finally {
    void invalidateTodo();
  }
  notifyUndo(movedMessage(to), () => {
    moveTask(task.id, from)
      .catch(() => notifyError("Couldn't move the Task back"))
      .finally(() => void invalidateTodo());
  });
}

/**
 * Brings a done Task back to open, in its old place and on its old date. An occurrence of a
 * repeating Task rewinds the Task, if it is the latest; an older one says why it cannot.
 */
export async function untickTask(task: Task): Promise<void> {
  try {
    await reopenTask(task.id);
  } catch (e) {
    notifyError(e instanceof IpcError ? e.message : "Couldn't reopen the Task");
  } finally {
    void invalidateTodo();
  }
}

/** Puts dates back as they were: Undo for a date change. */
function undoWhen(changes: DueChange[], failure: string): void {
  restoreWhen(changes)
    .catch(() => notifyError(failure))
    .finally(() => void invalidateTodo());
}

/**
 * Gives a Task a new date, time and Repeat, or none. "“Pay rent” → Tomorrow · 09:00" offers Undo,
 * which puts the old date and Repeat back even when the date is in the past. Rust refuses a past
 * date; the reason shows. `repeat` is the Repeat after the change: pass `taskRepeat(task)` to keep it.
 */
export async function setTaskWhen(
  task: Task,
  date: string | null,
  time: string | null,
  repeat: RepeatRule | null,
): Promise<void> {
  const before: DueChange = {
    id: task.id,
    due_date: task.due_date,
    due_time: task.due_time,
    repeat: taskRepeat(task),
  };
  let after: Task;
  try {
    after = await setWhen(task.id, date, time, repeat);
  } catch (e) {
    notifyError(e instanceof IpcError ? e.message : "Couldn't change the date");
    return;
  } finally {
    void invalidateTodo();
  }
  const now = taskRepeat(after);
  if (
    after.due_date === before.due_date &&
    after.due_time === before.due_time &&
    JSON.stringify(now) === JSON.stringify(before.repeat)
  ) {
    return;
  }
  const message = whenMessage(
    task.title,
    after.due_date,
    after.due_time,
    todoClock.today,
    todoPrefs.timeFormat,
    now,
    before.repeat !== null,
  );
  // Changing the same Task's When again replaces its toast instead of stacking them.
  notifyUndo(message, () => undoWhen([before], "Couldn't undo the date change"), `when-${task.id}`);
}

/** The ☀ on a row: due today, keeping its time and its Repeat. */
export const dateTaskToday = (task: Task): Promise<void> =>
  setTaskWhen(task, todoClock.today, task.due_time, taskRepeat(task));

/** "Skip to ‹date›": a repeating Task moves to its next date and records nothing. Undo moves it back. */
export async function skipRepeating(task: Task): Promise<void> {
  const before: DueChange = {
    id: task.id,
    due_date: task.due_date,
    due_time: task.due_time,
    repeat: taskRepeat(task),
  };
  let after: Task;
  try {
    after = await skipTask(task.id);
  } catch (e) {
    notifyError(e instanceof IpcError ? e.message : "Couldn't skip the Task");
    return;
  } finally {
    void invalidateTodo();
  }
  if (after.due_date === null) return;
  notifyUndo(skippedMessage(task.title, after.due_date, todoClock.today), () =>
    undoWhen([before], "Couldn't undo the skip"),
  );
}

/** Moves every Overdue Task to `date` (today or a picked day); Undo puts each one back. */
export async function moveAllOverdue(date: string): Promise<void> {
  let moved: Awaited<ReturnType<typeof moveOverdue>>;
  try {
    moved = await moveOverdue(date);
  } catch (e) {
    notifyError(e instanceof IpcError ? e.message : "Couldn't move the Tasks");
    return;
  } finally {
    void invalidateTodo();
  }
  if (moved.previous.length === 0) return;
  notifyUndo(movedAllMessage(moved.previous.length, date, todoClock.today), () =>
    undoWhen(moved.previous, "Couldn't undo the move"),
  );
}
