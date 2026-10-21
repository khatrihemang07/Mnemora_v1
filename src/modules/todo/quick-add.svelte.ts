import { todoSearch } from './search.svelte';

/**
 * Q, and the top bar's "New task". Inside Todo the box opens in place: the first "+ New task" row
 * on the page is clicked, so the Task inherits that place (Today, a List, an Upcoming day). Where
 * the page has no such row, and everywhere outside Todo, the floating Quick add opens (Inbox).
 */
class QuickAdd {
  /** The floating box is open. */
  floating = $state(false);

  /** `inTodo`: the shell says Todo is the page on screen. */
  request(inTodo: boolean): void {
    // While search covers the page, its rows are not there to click: the floating box opens.
    if (inTodo && !todoSearch.active && typeof document !== 'undefined') {
      const open = document.querySelector<HTMLInputElement>('[data-capture-box] input');
      if (open) {
        open.focus();
        return;
      }
      const row = document.querySelector<HTMLElement>('[data-new-task]');
      if (row) {
        row.click();
        return;
      }
    }
    this.floating = true;
  }

  close(): void {
    this.floating = false;
  }
}

export const quickAdd = new QuickAdd();
