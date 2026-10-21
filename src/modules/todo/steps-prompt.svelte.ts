import type { Task } from '../../lib/ipc/bindings';

/**
 * The question "N steps are still open" while a Task with unticked Checklist items is being
 * completed. Completing asks every time; the Dialog in Todo's page answers it.
 */
class StepsPrompt {
  /** The Task being completed, or none when nothing is asked. */
  task = $state.raw<Task | null>(null);

  ask(task: Task): void {
    this.task = task;
  }

  close(): void {
    this.task = null;
  }
}

export const stepsPrompt = new StepsPrompt();
