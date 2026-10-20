import { getContext, setContext } from 'svelte';
import type { WeekStart } from '../../lib/ui/calendar';

/** How long a flashed Entry or header stays tinted; a touch under its fade, so no frame lingers. */
const FLASH_MS = 1550;

/**
 * Where the Journal is pointing, shared by the feed, the composer and the Entry actions through
 * context: the Calendar's week start, which Entry or Page header is flashing, the date the composer's chip shows, and the
 * feed's own scroller (registered by `JournalPage`, so actions can reach it without importing it).
 */
export class JournalNav {
  /** The Entry that just arrived somewhere (moved, or found through Show). */
  flashEntry = $state<string | null>(null);
  /** The Page header being pointed at (a calendar day was tapped). */
  flashPage = $state<string | null>(null);
  /** First day of the week, from the General Setting (the Calendars use it). */
  weekStart = $state<WeekStart>('mon');
  /** The composer's date chip; null means Today. */
  composerDate = $state<string | null>(null);
  /** Scrolls to a Page, loading older Pages if needed, and flashes it or one of its Entries. */
  scrollTo: ((date: string, entryId?: string) => Promise<void>) | null = null;
  /** Focuses the composer. */
  focusComposer: (() => void) | null = null;
  /** ⌘N: focuses the composer, on Today unless text is already being written for another day. */
  compose: (() => void) | null = null;
  #entryTimer: ReturnType<typeof setTimeout> | undefined;
  #pageTimer: ReturnType<typeof setTimeout> | undefined;

  constructor(public today: string) {}

  /** Tints an Entry for a moment, restarting if it is already tinted. */
  flashEntryFor(id: string): void {
    clearTimeout(this.#entryTimer);
    this.flashEntry = null;
    queueMicrotask(() => {
      this.flashEntry = id;
      this.#entryTimer = setTimeout(() => (this.flashEntry = null), FLASH_MS);
    });
  }

  /** Tints a Page header for a moment. */
  flashPageFor(date: string): void {
    clearTimeout(this.#pageTimer);
    this.flashPage = null;
    queueMicrotask(() => {
      this.flashPage = date;
      this.#pageTimer = setTimeout(() => (this.flashPage = null), FLASH_MS);
    });
  }

  /** "Write on this day": the composer's chip moves to `date` and the composer takes focus. */
  writeOn(date: string): void {
    this.composerDate = date;
    this.focusComposer?.();
  }

  /** Drops pending timers when the Journal closes. */
  dispose(): void {
    clearTimeout(this.#entryTimer);
    clearTimeout(this.#pageTimer);
    this.scrollTo = null;
    this.focusComposer = null;
    this.compose = null;
  }
}

const KEY = Symbol('journal-nav');
export const provideNav = (n: JournalNav) => setContext(KEY, n);
export const useNav = () => getContext<JournalNav>(KEY);
