import { todoSearch } from './search.svelte';
import { moveRing, ringAfterRemoval } from './task-keymap';

/** The element a page marks as the Tasks the keyboard walks: the view, or the search results. */
const SCOPE = '[data-task-scope]';

/**
 * The keyboard focus ring across the Tasks on screen (desktop): `id` is the Task it sits on. The
 * rows come from the DOM in reading order, so it works the same on every view, group and the
 * search results. Moving gives the row's title real focus, so Tab, a screen reader and the keys
 * on a focused Task all agree about where you are.
 */
class TaskFocus {
  id = $state<string | null>(null);

  /** The Tasks on screen, top to bottom: the results while search is open, else the view. */
  ids(): string[] {
    const scope = document.querySelector(
      `[data-task-scope="${todoSearch.active ? 'results' : 'view'}"]`,
    );
    if (!scope) return [];
    return [...scope.querySelectorAll<HTMLElement>('[data-task-row]')]
      .filter((el) => el.getClientRects().length > 0)
      .map((el) => el.dataset.taskRow ?? '');
  }

  /** ↑ ↓: moves the ring and the focus. */
  move(dir: 'up' | 'down'): void {
    const ids = this.ids();
    const current = this.id !== null && ids.includes(this.id) ? this.id : null;
    const next = moveRing(ids, current, dir);
    if (next !== null) this.focus(next);
  }

  /** Puts the ring and the focus on a Task's row. */
  focus(id: string): void {
    this.id = id;
    const title = rowTitle(id);
    title?.focus({ preventScroll: true });
    title?.scrollIntoView({ block: 'nearest' });
  }

  /** Drops the ring; the focus is let go of when it is on a row. */
  clear(): void {
    this.id = null;
    const active = document.activeElement;
    if (active instanceof HTMLElement && active.closest('[data-task-row]')) active.blur();
  }

  /**
   * Keeps the ring going after something that may move or remove the Task (done, deleted, moved,
   * dated): once the page has redrawn, it stays on the Task if it is still there, else it goes to
   * `fallback` (the Task that was next to it), else it is dropped.
   */
  keepAfter(id: string, fallback: string | null): void {
    requestAnimationFrame(() =>
      requestAnimationFrame(() => {
        const ids = this.ids();
        if (ids.includes(id)) this.focus(id);
        else if (fallback !== null && ids.includes(fallback)) this.focus(fallback);
        else this.id = null;
      }),
    );
  }

  /** The Task the ring goes to if `id` goes away. */
  neighbour(id: string): string | null {
    return ringAfterRemoval(this.ids(), id);
  }
}

const rowTitle = (id: string): HTMLElement | null =>
  document.querySelector<HTMLElement>(`[data-task-row="${CSS.escape(id)}"] [role="button"]`);

export const taskFocus = new TaskFocus();
export { SCOPE };
