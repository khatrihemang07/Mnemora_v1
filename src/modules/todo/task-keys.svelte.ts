import { isTypingTarget } from '../../lib/bare-key';
import type { Task } from '../../lib/ipc/bindings';

/**
 * The rows on screen, for the keys that act on the focused row. Each row registers its Task while it
 * is mounted.
 */
class TaskRows {
  #rows = new Map<string, { task: Task; pickers: RowPickers }>();

  /** `pickers` open the row's own pickers, for the keys D and M. */
  register(task: Task, pickers: RowPickers): () => void {
    const entry = { task, pickers };
    this.#rows.set(task.id, entry);
    return () => {
      if (this.#rows.get(task.id) === entry) this.#rows.delete(task.id);
    };
  }

  get(id: string | null | undefined): Task | null {
    return (id ? this.#rows.get(id)?.task : null) ?? null;
  }

  pickers(id: string): RowPickers | null {
    return this.#rows.get(id)?.pickers ?? null;
  }
}

/** What a row can open for the keyboard. */
export interface RowPickers {
  when: () => void;
  moveToList: () => void;
}

export const taskRows = new TaskRows();

/** Whether the key is going into a text field, where Todo's single-key shortcuts must stay out. */
export const isTyping = isTypingTarget;

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

const OVERLAYS = '[role="menu"], [role="listbox"], [role="dialog"], [role="alertdialog"]';

/**
 * Whether a menu, picker or Dialog is open (ADR 0011). With the Task sheet open, the sheet is itself
 * a dialog and does not count; anything opened over it does.
 */
export function overlayOpen(sheetOpen: boolean): boolean {
  return [...document.querySelectorAll(OVERLAYS)].some(
    (el) => !(sheetOpen && el.querySelector('[data-task-sheet]')),
  );
}
