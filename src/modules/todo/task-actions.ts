import type { DueChange, Task, TodoList } from '../../lib/ipc/bindings';
import { IpcError } from '../../lib/ipc/unwrap';
import { notifyError, notifyUndo } from '../../lib/ui/toast';
import { todoClock } from './clock.svelte';
import { movedAllMessage, whenMessage } from './dates';
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
} from './queries';
import { completedMessage } from './tasks';

/**
 * Ticks a Task: it leaves the page at once, then "Completed “…”" offers Undo, which reopens it in
 * its old place. A failed write brings the Task back from Rust's truth.
 */
export async function tickTask(task: Task): Promise<void> {
  removeCachedTask(task.id);
  try {
    await completeTask(task.id);
  } catch {
    void invalidateTodo();
    notifyError("Couldn't complete the Task");
    return;
  }
  void invalidateTodo();
  notifyUndo(completedMessage(task.title), () => {
    reopenTask(task.id)
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

/** Brings a done Task back to open, in its old place and on its old date. */
export async function untickTask(task: Task): Promise<void> {
  try {
    await reopenTask(task.id);
  } catch {
    notifyError("Couldn't reopen the Task");
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
 * Gives a Task a new date and time, or none. "“Pay rent” → Tomorrow · 09:00" offers Undo, which
 * puts the old date back even when it is in the past. Rust refuses a past date; the reason shows.
 */
export async function setTaskWhen(
  task: Task,
  date: string | null,
  time: string | null,
): Promise<void> {
  const before: DueChange = { id: task.id, due_date: task.due_date, due_time: task.due_time };
  let after: Task;
  try {
    after = await setWhen(task.id, date, time);
  } catch (e) {
    notifyError(e instanceof IpcError ? e.message : "Couldn't change the date");
    return;
  } finally {
    void invalidateTodo();
  }
  if (after.due_date === before.due_date && after.due_time === before.due_time) return;
  const message = whenMessage(
    task.title,
    after.due_date,
    after.due_time,
    todoClock.today,
    todoPrefs.timeFormat,
  );
  notifyUndo(message, () => undoWhen([before], "Couldn't undo the date change"));
}

/** The ☀ on a row: due today, keeping its time. */
export const dateTaskToday = (task: Task): Promise<void> =>
  setTaskWhen(task, todoClock.today, task.due_time);

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
