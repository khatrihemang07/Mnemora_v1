import { SEARCH_DEBOUNCE_MS } from './search-results';

/** The palette hands focus back as it closes; a field that wants it takes it after this. */
export const PALETTE_CLOSE_MS = 80;

/**
 * Todo's search session. `query` is what the field holds; `committed` is what the results follow:
 * it trails typing by 150ms and follows every other change at once. A Task opened from the
 * results shows its sheet over them (the sheet is the URL's business), so the session simply stays
 * as it is and Esc on the sheet lands back on the same results.
 */
class TodoSearch {
  active = $state(false);
  query = $state('');
  committed = $state('');
  #timer: ReturnType<typeof setTimeout> | undefined;

  /** Opens search, holding `query` (blank by default). */
  open(query?: string): void {
    this.active = true;
    if (query !== undefined) this.#set(query);
  }

  /** Leaves search for good: the next one starts blank. */
  close(): void {
    this.active = false;
    this.#set('');
  }

  /** Typing: the field follows at once, the results after a pause. */
  setQuery(query: string): void {
    this.query = query;
    clearTimeout(this.#timer);
    this.#timer = setTimeout(() => (this.committed = query), SEARCH_DEBOUNCE_MS);
  }

  /** Esc in the field: clears the words, then closes. Resolves true when it closed. */
  escape(): boolean {
    if (this.query !== '') {
      this.#set('');
      return false;
    }
    this.close();
    return true;
  }

  #set(query: string): void {
    clearTimeout(this.#timer);
    this.query = query;
    this.committed = query;
  }
}

export const todoSearch = new TodoSearch();

/** A search asked for from ⌘K; Todo takes it once its page is up. */
class SearchLauncher {
  pending = $state<string | null>(null);
}
export const searchLauncher = new SearchLauncher();

/** Opens Todo's search holding `query` (empty for a blank one). */
export const requestSearch = (query: string): void => {
  searchLauncher.pending = query;
};
