import { isTypingTarget } from '../../lib/bare-key';
import type { Task } from '../../lib/ipc/bindings';

/**
 * The rows on screen, for the keys that act on "the row under the pointer or with focus". Each row
 * registers its Task while it is mounted; the hovered row is the one under the pointer.
 */
class TaskRows {
  #rows = new Map<string, Task>();
  hoveredId = $state<string | null>(null);

  register(task: Task): () => void {
    this.#rows.set(task.id, task);
    return () => {
      if (this.#rows.get(task.id) === task) this.#rows.delete(task.id);
      if (this.hoveredId === task.id) this.hoveredId = null;
    };
  }

  get(id: string | null | undefined): Task | null {
    return (id ? this.#rows.get(id) : null) ?? null;
  }
}

export const taskRows = new TaskRows();

/** Whether the key is going into a text field, where Todo's single-key shortcuts must stay out. */
export const isTyping = isTypingTarget;

/**
 * The Priority a plain 1 to 4 key means, or none: not with a modifier, not while typing, not
 * inside a menu or picker.
 */
export function priorityKey(e: KeyboardEvent): number | null {
  if (e.metaKey || e.ctrlKey || e.altKey || e.shiftKey) return null;
  return /^[1-4]$/.test(e.key) ? Number(e.key) : null;
}

/** ⇧Space: Won't do (Skip on a repeating Task). */
export const wontKey = (e: KeyboardEvent): boolean =>
  e.key === ' ' && e.shiftKey && !e.metaKey && !e.ctrlKey && !e.altKey;

/**
 * Whether a key aimed at `target` belongs to the open Task sheet: focus is in the sheet, on its
 * panel, or on the page behind it, and not in a menu, picker or Dialog opened over it.
 */
export function inSheetScope(target: EventTarget | null): boolean {
  const el = target instanceof Element ? target : null;
  const overlay = el?.closest(
    '[role="menu"], [role="listbox"], [role="dialog"], [role="alertdialog"]',
  );
  return !overlay || overlay.querySelector('[data-task-sheet]') !== null;
}
