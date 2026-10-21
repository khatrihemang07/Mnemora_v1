import type { Task } from '../../lib/ipc/bindings';
import { urlParam } from '../../lib/router/url-param';

const PARAM = 'task';
/** Closes asked for within this long of each other are one close. */
const CLOSE_DEBOUNCE_MS = 400;

/**
 * Which Task is open in the sheet. It lives in the URL (`/todo?task=‹id›`), so a reload or a link
 * opens it again and Android Back closes it: opening pushes a history entry, closing steps back
 * over it. Moving to the previous or next Task swaps the entry instead of piling them up. The
 * sheet itself reports the Task it shows (`task`), for the keys that act on it.
 */
class TaskSheetState {
  /** The open Task's id, or none. */
  readonly id = $derived(urlParam.get(PARAM));
  /** The open Task as the sheet last loaded it. */
  task = $state.raw<Task | null>(null);
  /** Whether opening pushed the entry that closing should step back over. */
  #pushed = false;
  /** When the last close began: Esc and the editor can both ask for one, and only the first counts. */
  #closedAt = 0;

  /** Opens a Task. Opening another while one is open swaps it. */
  open(id: string): void {
    if (this.id === id) return;
    if (this.id === null) this.#pushed = true;
    urlParam.set(PARAM, id, this.id !== null);
  }

  /** Switches to another Task without adding a history entry. */
  switchTo(id: string): void {
    if (this.id !== id && !this.#justClosed()) urlParam.set(PARAM, id, true);
  }

  #justClosed(): boolean {
    return Date.now() - this.#closedAt < CLOSE_DEBOUNCE_MS;
  }

  /** Closes the sheet: back over the entry opening pushed, else drop the parameter. */
  close(): void {
    if (this.id === null || this.#justClosed()) return;
    this.#closedAt = Date.now();
    if (this.#pushed) {
      this.#pushed = false;
      history.back();
    } else {
      urlParam.delete(PARAM, true);
    }
  }
}

export const taskSheet = new TaskSheetState();
