import type { Completion, DueChange, GroupKey, Task, TodoList } from '../../lib/ipc/bindings';
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
import type { Neighbours } from './drag-math';
import { movedMessage } from './lists';
import { openSteps, tickAllSteps } from './notes';
import { todoPrefs } from './prefs.svelte';
import {
  completeTask,
  deleteTask,
  invalidateTodo,
  moveOverdue,
  moveTask,
  removeCachedTask,
  reopenTask,
  reorderTask,
  restoreTask,
  restoreWhen,
  setPriority,
  setWhen,
  skipTask,
  updateTask,
  wontDo,
} from './queries';
import { stepsPrompt } from './steps-prompt.svelte';
import {
  completedMessage,
  deletedMessage,
  priorityMessage,
  quotedTitle,
  wontMessage,
} from './tasks';

/**
 * Ticks a Task. One with unticked Checklist items asks first, every time (the Dialog offers
 * Cancel, "Tick all and complete" and Complete); any other completes at once, see `completeNow`.
 */
export async function tickTask(task: Task): Promise<void> {
  if (task.state === 'open' && openSteps(task.notes) > 0) {
    stepsPrompt.ask(task);
    return;
  }
  await completeNow(task);
}

/** "Tick all and complete": the notes get every step ticked, then the Task is completed. */
export async function tickAllAndComplete(task: Task): Promise<void> {
  const notes = tickAllSteps(task.notes);
  try {
    await updateTask(task.id, task.title, notes, crypto.randomUUID());
  } catch (e) {
    notifyError(e instanceof IpcError ? e.message : "Couldn't tick the steps");
    return;
  }
  await completeNow({ ...task, notes });
}

/**
 * Completes a Task without asking: it leaves the page at once, then "Completed “…”" offers Undo,
 * which reopens it in its old place. A repeating Task stays: the toast says where it went
 * ("· next Sat 24 Oct") and Undo reopens that occurrence, which rewinds the Task. A failed write
 * brings the Task back from Rust's truth. Returns the Task as it is now, or none on failure.
 */
export async function completeNow(task: Task): Promise<Task | null> {
  const repeating = task.repeat_freq !== null;
  if (!repeating) removeCachedTask(task.id);
  let done: Completion;
  try {
    done = await completeTask(task.id);
  } catch {
    void invalidateTodo();
    notifyError("Couldn't complete the Task");
    return null;
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
  return done.task;
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

/**
 * Sets a Task's Priority (1 to 4). "“Pay rent” → P1" offers Undo; changing it again replaces the
 * toast instead of stacking them.
 */
export async function setTaskPriority(task: Task, priority: number): Promise<void> {
  if (task.priority === priority) return;
  const before = task.priority;
  try {
    await setPriority(task.id, priority);
  } catch (e) {
    notifyError(e instanceof IpcError ? e.message : "Couldn't change the Priority");
    return;
  } finally {
    void invalidateTodo();
  }
  notifyUndo(
    priorityMessage(task.title, priority),
    () => {
      setPriority(task.id, before)
        .catch(() => notifyError("Couldn't undo the Priority change"))
        .finally(() => void invalidateTodo());
    },
    `priority-${task.id}`,
  );
}

/**
 * Won't do: the Task joins Done with a ⊗ and Undo reopens it. On a repeating Task it is Skip
 * (the next date, nothing recorded).
 */
export async function wontDoTask(task: Task): Promise<void> {
  if (task.state !== 'open') return;
  if (task.repeat_freq !== null) {
    await skipRepeating(task);
    return;
  }
  removeCachedTask(task.id);
  try {
    await wontDo(task.id);
  } catch (e) {
    void invalidateTodo();
    notifyError(e instanceof IpcError ? e.message : "Couldn't mark the Task Won't do");
    return;
  }
  void invalidateTodo();
  notifyUndo(wontMessage(task.title), () => {
    reopenTask(task.id)
      .catch(() => notifyError("Couldn't reopen the Task"))
      .finally(() => void invalidateTodo());
  });
}

/** Deletes a Task at once; the toast names it and Undo brings it back. Resolves true on success. */
export async function deleteTaskWithUndo(task: Task): Promise<boolean> {
  removeCachedTask(task.id);
  try {
    await deleteTask(task.id);
  } catch {
    void invalidateTodo();
    notifyError("Couldn't delete the Task");
    return false;
  }
  void invalidateTodo();
  notifyUndo(deletedMessage(task.title), () => {
    restoreTask(task.id)
      .catch((e) => notifyError(e instanceof IpcError ? e.message : "Couldn't restore the Task"))
      .finally(() => void invalidateTodo());
  });
  return true;
}

/** Where a dragged Task lands: between two neighbours, in the group that adopts `adopt`. */
export interface Placement extends Neighbours {
  /** What the group adopts (see `TaskGroup.adopt`): a day, a Priority or a List. */
  adopt: GroupKey[];
}

/**
 * Puts a Task in a place of the shared manual order, and in the group it was dropped into: that
 * group's day, Priority or List becomes the Task's. When a property changed, "“Pay rent” → Sat"
 * (`summary` names it) offers Undo, which puts the properties and the old place back (even a past
 * date). Moving within a group changes only the order and says nothing. `from` is where it was.
 */
export async function placeTask(
  task: Task,
  to: Placement,
  from: Neighbours,
  summary: string | null,
): Promise<void> {
  let after: Task;
  try {
    after = await reorderTask(task.id, to.before, to.after, to.adopt);
  } catch (e) {
    notifyError(e instanceof IpcError ? e.message : "Couldn't move the Task");
    return;
  } finally {
    void invalidateTodo();
  }
  const changed =
    after.due_date !== task.due_date ||
    after.due_time !== task.due_time ||
    after.priority !== task.priority ||
    after.list_id !== task.list_id;
  if (!changed || summary === null) return;
  notifyUndo(
    `${quotedTitle(task.title)} → ${summary}`,
    () => {
      void undoPlacement(task, from);
    },
    `drop-${task.id}`,
  );
}

/** Undo for `placeTask`: the old date, Priority and List, then the old place. */
async function undoPlacement(before: Task, from: Neighbours): Promise<void> {
  try {
    await restoreWhen([
      {
        id: before.id,
        due_date: before.due_date,
        due_time: before.due_time,
        repeat: taskRepeat(before),
      },
    ]);
    await setPriority(before.id, before.priority);
    await moveTask(before.id, before.list_id);
    await reorderTask(before.id, from.before, from.after, []);
  } catch {
    notifyError("Couldn't undo the move");
  } finally {
    void invalidateTodo();
  }
}

/** Dropping a Task on the sidebar: a List (or the Inbox) moves it there, Today dates it today. All with Undo. */
export async function dropTaskOnSidebar(
  task: Task,
  target: { kind: 'inbox' } | { kind: 'today' } | { kind: 'list'; list: TodoList },
): Promise<void> {
  if (target.kind === 'today') await dateTaskToday(task);
  else await moveTaskTo(task, target.kind === 'inbox' ? null : target.list);
}
