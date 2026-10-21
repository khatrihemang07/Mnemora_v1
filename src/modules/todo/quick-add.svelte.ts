import { todoSearch } from './search.svelte';

/**
 * Q, and the top bar's "New task". Inside Todo the box opens in place: the first "+ New task" row
 * on the page is clicked, so the Task inherits that place (Today, a List, an Upcoming day). Where
 * the page has no such row, and everywhere outside Todo, the floating Quick add opens (Inbox).
 */
class QuickAdd {
  /** The floating box is open. */
  floating = $state(false);
  /** The day a Task added in the floating box is due: Home's Add button opens it on today. */
  date = $state<string | null>(null);
  /** The List a Task added in the floating box goes to (the phone's + inside a List). */
  listId = $state<string | null>(null);

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
    this.openFloating(null);
  }

  /**
   * Opens the floating box, optionally with a day its Tasks are due (Home's Add: today) and a List
   * they go to (the phone's + button takes both from the view it is pressed in).
   */
  openFloating(date: string | null, listId: string | null = null): void {
    this.date = date;
    this.listId = listId;
    this.floating = true;
  }

  close(): void {
    this.floating = false;
    this.date = null;
    this.listId = null;
  }
}

export const quickAdd = new QuickAdd();
