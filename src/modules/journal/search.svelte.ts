import {
  closeSearch,
  escapeSearch,
  goBack,
  IDLE,
  openResult,
  openSearch,
  openSearchFor,
  SEARCH_DEBOUNCE_MS,
  type SearchRange,
  type SearchSession,
  toggleSearch,
  withQuery,
  withRange,
} from './search-state';

/**
 * The Journal's search: the session state (see `search-state.ts`) plus the debounce. `committed` is
 * the query the results follow: it trails typing by 150ms, and follows every other change at once.
 */
export class JournalSearch {
  session = $state.raw<SearchSession>(IDLE);
  committed = $state('');
  /** Where the results list was scrolled when it was left through a result. */
  savedScroll = 0;
  #timer: ReturnType<typeof setTimeout> | undefined;

  get active() {
    return this.session.active;
  }
  get parked() {
    return this.session.parked !== null;
  }
  get query() {
    return this.session.filter.query;
  }
  get range() {
    return this.session.filter.range;
  }

  #set(next: SearchSession) {
    clearTimeout(this.#timer);
    this.session = next;
    this.committed = next.filter.query;
    if (!next.active && next.parked === null) this.savedScroll = 0;
  }

  open() {
    this.#set(openSearch(this.session));
  }
  openFor(query: string) {
    this.#set(openSearchFor(query));
  }
  /** The header button: opens, or leaves search for good (parked results included). */
  toggle() {
    this.#set(toggleSearch(this.session));
  }
  close() {
    this.#set(closeSearch());
  }
  /** Opening a result: search closes but Back or ⌘F returns to it. */
  park(scrollTop: number) {
    this.savedScroll = scrollTop;
    this.#set(openResult(this.session));
  }
  /** Back: resumes parked results, else leaves search. */
  back() {
    this.#set(goBack(this.session));
  }
  escape() {
    this.#set(escapeSearch(this.session));
  }
  setRange(range: SearchRange | null) {
    this.#set(withRange(this.session, range));
  }
  /** Typing: the field follows at once, the results after a pause. */
  setQuery(query: string) {
    this.session = withQuery(this.session, query);
    clearTimeout(this.#timer);
    this.#timer = setTimeout(() => (this.committed = query), SEARCH_DEBOUNCE_MS);
  }
  dispose() {
    clearTimeout(this.#timer);
  }
}

/** A search asked for from outside the Journal (⌘K); the Journal takes it when it is showing. */
class SearchLauncher {
  pending = $state<string | null>(null);
}
export const searchLauncher = new SearchLauncher();

/** Opens the Journal's search holding `query` (empty for a blank one). */
export const requestSearch = (query: string) => {
  searchLauncher.pending = query;
};
