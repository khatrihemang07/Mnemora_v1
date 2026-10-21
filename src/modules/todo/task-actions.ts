import type { Task, TodoList } from '../../lib/ipc/bindings';
import { notifyError, notifyUndo } from '../../lib/ui/toast';
import { movedMessage } from './lists';
import { completeTask, invalidateTodo, moveTask, removeCachedTask, reopenTask } from './queries';
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
