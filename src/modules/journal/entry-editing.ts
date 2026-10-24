import { SAVE_PAUSE_MS } from './composer';

export interface EntryEditingDeps<E> {
  update: (id: string, text: string, sessionId: string) => Promise<E>;
  /** Starts a timer and returns its canceller. */
  setTimer: (fn: () => void, ms: number) => () => void;
  /** Reports each save's outcome: the stored Entry, or the error that left text unsaved. */
  onSaved: (entry: E) => void;
  onError: (error: unknown) => void;
}

/**
 * One editing session of an existing Entry: typing autosaves after a pause, every save carries the
 * session id (the change log keeps one record per session), and a failed save keeps the text and
 * simply tries again on the next edit or Retry. Saves run one after another. Blank text is never
 * saved (an Entry can't be blank), so clearing everything and leaving keeps the last saved text.
 */
export class EntryEditing<E> {
  text: string;
  #saved: string;
  #cancelTimer: (() => void) | null = null;
  #queue: Promise<void> = Promise.resolve();
  #abandoned = false;

  constructor(
    private readonly deps: EntryEditingDeps<E>,
    readonly id: string,
    initial: string,
    readonly sessionId: string,
  ) {
    this.text = initial;
    this.#saved = initial;
  }

  /** Text that differs from what is stored and could be saved. */
  get unsaved(): boolean {
    return this.text.trim() !== '' && this.text !== this.#saved;
  }

  /** Blank text over a stored Entry: clearing never deletes an Entry, so the previous text stays. */
  get blankKept(): boolean {
    return this.text.trim() === '' && this.text !== this.#saved;
  }

  /** The text changed (every keystroke). */
  input(text: string): void {
    this.text = text;
    this.#cancelTimer?.();
    this.#cancelTimer = this.unsaved ? this.deps.setTimer(() => this.#save(), SAVE_PAUSE_MS) : null;
  }

  /** Saves what is pending now. True when nothing unsaved remains. */
  async flush(): Promise<boolean> {
    this.#cancelTimer?.();
    this.#cancelTimer = null;
    this.#save();
    await this.#queue;
    return this.#abandoned || !this.unsaved;
  }

  /**
   * The Entry was deleted elsewhere: nothing more is saved by this session. Returns the text it
   * holds, and the text last saved, which the person may Restore with.
   */
  abandon(): { text: string; saved: string } {
    this.#abandoned = true;
    this.#cancelTimer?.();
    this.#cancelTimer = null;
    return { text: this.text, saved: this.#saved };
  }

  #save() {
    this.#queue = this.#queue.then(async () => {
      const text = this.text;
      if (this.#abandoned || !this.unsaved) return;
      try {
        const entry = await this.deps.update(this.id, text, this.sessionId);
        this.#saved = text;
        this.deps.onSaved(entry);
      } catch (e) {
        this.deps.onError(e);
      }
    });
  }
}
