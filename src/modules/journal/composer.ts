/** Quiet time after the last keystroke before an autosave (S1). */
export const SAVE_PAUSE_MS = 600;

export type ComposerPhase = 'idle' | 'creating' | 'editing' | 'finishing';

export interface ComposerDeps {
  /** `pageDate` is the date chip's choice; null means the default (Today). */
  create: (text: string, sessionId: string, pageDate: string | null) => Promise<{ id: string }>;
  update: (id: string, text: string, sessionId: string) => Promise<unknown>;
  /** Moves the Entry already created to another Page. */
  move: (id: string, pageDate: string) => Promise<unknown>;
  /** Deletes the Entry already created, once its text was cleared and the composer finished. */
  discard: (id: string) => Promise<unknown>;
  /** Starts a timer and returns its canceller. */
  setTimer: (fn: () => void, ms: number) => () => void;
  newSessionId: () => string;
  onError: (error: unknown) => void;
}

/**
 * The composer's life: idle → creating → editing → finishing → idle. The first visible character
 * creates the Entry, later typing updates it after a pause, finishing flushes and starts over.
 * A blank composer never creates anything, and clearing an Entry already created then finishing
 * discards it. While the box keeps focus, blank text only waits (retyping reuses the same Entry).
 * All saves run one after another.
 */
export class Composer {
  phase: ComposerPhase = 'idle';
  text = '';
  /** Id of the Entry the last `finish()` completed, for the feed to welcome it. */
  lastFinishedId: string | null = null;
  /** The Entry being written, once created. */
  get entryId(): string | null {
    return this.#entryId;
  }
  /** The Page the Entry goes on; null is Today. Choosing it after creation moves the Entry. */
  pageDate: string | null = null;
  #sessionId: string;
  #entryId: string | null = null;
  #saved = '';
  #cancelTimer: (() => void) | null = null;
  #queue: Promise<void> = Promise.resolve();

  constructor(private readonly deps: ComposerDeps) {
    this.#sessionId = deps.newSessionId();
  }

  /** The text changed (every keystroke). */
  input(text: string): void {
    this.text = text;
    if (this.phase === 'finishing') return;
    if (this.phase === 'idle') {
      if (text.trim()) this.#create();
      return;
    }
    this.#schedule();
  }

  /**
   * Saves what is pending and starts a fresh session; blank text discards the Entry already
   * created. Returns false, keeping the text, when a save failed so nothing is lost.
   */
  async finish(): Promise<boolean> {
    if (this.phase === 'finishing') return false;
    this.#cancelTimer?.();
    this.#cancelTimer = null;
    if (this.phase === 'idle' && this.text.trim()) this.#create();
    else this.#save();
    this.phase = 'finishing';
    await this.#queue;

    const id = this.#entryId;
    if (!this.text.trim()) {
      if (id !== null) {
        try {
          await this.deps.discard(id);
        } catch (e) {
          this.deps.onError(e);
          this.phase = 'editing';
          return false;
        }
      }
      this.lastFinishedId = null;
      this.#reset();
      return true;
    }
    if (id === null || this.text !== this.#saved) {
      this.phase = id === null ? 'idle' : 'editing';
      return false;
    }
    this.lastFinishedId = id;
    this.#reset();
    return true;
  }

  /** The date chip changed: new Entries go there, and one already created moves there. */
  setPageDate(date: string | null): void {
    if (date === this.pageDate) return;
    this.pageDate = date;
    if (date === null) return;
    this.#enqueue(async () => {
      if (this.#entryId === null) return;
      try {
        await this.deps.move(this.#entryId, date);
      } catch (e) {
        this.deps.onError(e);
      }
    });
  }

  /** Text that is not stored yet: a failed create, or edits a failed update left behind. */
  get unsaved(): boolean {
    if (!this.text.trim()) return false;
    return this.#entryId === null ? this.phase !== 'creating' : this.text !== this.#saved;
  }

  /** Tries the pending save again now (Retry, blur, leaving). */
  async saveNow(): Promise<void> {
    this.#cancelTimer?.();
    this.#cancelTimer = null;
    if (this.phase === 'idle' && this.text.trim()) this.#create();
    else this.#save();
    await this.#queue;
  }

  #schedule() {
    this.#cancelTimer?.();
    this.#cancelTimer = this.deps.setTimer(() => this.#save(), SAVE_PAUSE_MS);
  }

  #reset() {
    this.phase = 'idle';
    this.text = '';
    this.pageDate = null;
    this.#entryId = null;
    this.#saved = '';
    this.#sessionId = this.deps.newSessionId();
  }

  #create() {
    this.phase = 'creating';
    const text = this.text;
    this.#enqueue(async () => {
      try {
        const entry = await this.deps.create(text, this.#sessionId, this.pageDate);
        this.#entryId = entry.id;
        this.#saved = text;
        if (this.phase === 'creating') this.phase = 'editing';
        if (this.text !== text) this.#schedule();
      } catch (e) {
        if (this.phase === 'creating') this.phase = 'idle';
        this.deps.onError(e);
      }
    });
  }

  #save() {
    this.#enqueue(async () => {
      const text = this.text;
      if (this.#entryId === null || text === this.#saved || !text.trim()) return;
      try {
        await this.deps.update(this.#entryId, text, this.#sessionId);
        this.#saved = text;
      } catch (e) {
        this.deps.onError(e);
      }
    });
  }

  #enqueue(job: () => Promise<void>) {
    this.#queue = this.#queue.then(job);
  }
}
