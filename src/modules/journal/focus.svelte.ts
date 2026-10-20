import { getContext, setContext } from 'svelte';
import {
  type FeedEntryRef,
  type FocusEvent,
  type FocusState,
  focusReducer,
  NO_FOCUS,
} from './focus-nav';

/**
 * The keyboard focus across the feed's Entries (desktop). The state is the pure reducer in
 * `focus-nav.ts`; this wires it to the loaded Pages and to the DOM through callbacks that
 * `JournalPage` registers, so rows can ask for moves without importing the page.
 */
export class EntryFocus {
  state = $state.raw<FocusState>(NO_FOCUS);
  /** The loaded Entries in feed order. */
  entries: () => FeedEntryRef[] = () => [];
  /** Today's date, for ↑ from the composer. */
  today = '';
  /** Scrolls to the Entry (loading its Page into view if needed), gives its row focus. */
  reveal: ((id: string) => Promise<void>) | null = null;
  /** Loads older Pages; resolves when they have arrived. */
  loadOlder: (() => Promise<void>) | null = null;
  /** Brings Today's Pages back when the feed was opened on a far date. */
  toToday: (() => Promise<void>) | null = null;

  get id(): string | null {
    return this.state.id;
  }

  send(event: FocusEvent): void {
    this.state = focusReducer(this.state, event, this.entries());
  }

  /** ↑/↓: moves the ring, loading older Pages when ↓ runs past the end. */
  async move(direction: 'up' | 'down'): Promise<void> {
    this.send({ type: direction });
    if (this.state.wantsMore) {
      await this.loadOlder?.();
      this.send({ type: 'down' });
    }
    if (this.state.id !== null) await this.reveal?.(this.state.id);
  }

  /** ↑ from an empty composer. */
  async fromComposer(): Promise<void> {
    await this.toToday?.();
    this.send({ type: 'from-composer', today: this.today });
    if (this.state.id !== null) await this.reveal?.(this.state.id);
  }

  dispose(): void {
    this.reveal = null;
    this.loadOlder = null;
    this.toToday = null;
  }
}

const KEY = Symbol('journal-focus');
export const provideFocus = (f: EntryFocus) => setContext(KEY, f);
export const useFocus = () => getContext<EntryFocus>(KEY);
