import type { Task } from '../../lib/ipc/bindings';
import { notifyError, notifyUndo } from '../../lib/ui/toast';
import { completeTask, invalidateTodo, removeCachedTask, reopenTask } from './queries';
import { completedMessage } from './tasks';

/**
 * Ticks a Task: it leaves the Inbox at once, then "Completed “…”" offers Undo, which reopens it in
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
