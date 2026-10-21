/** Quiet time after the last keystroke before an autosave: the Journal's pause (S1). */
export const SAVE_PAUSE_MS = 600;

export interface TaskEditingDeps<T> {
  update: (id: string, title: string, notes: string, sessionId: string) => Promise<T>;
  /** Starts a timer and returns its canceller. */
  setTimer: (fn: () => void, ms: number) => () => void;
  /** Reports each save's outcome: the stored Task, or the error that left text unsaved. */
  onSaved: (task: T) => void;
  onError: (error: unknown) => void;
}

/** A pasted line break becomes a space: a title is one line. */
const oneLine = (text: string): string => text.replace(/\s*[\r\n]+\s*/g, ' ');

/**
 * One editing session of a Task's title and notes: typing autosaves after a pause, every save
 * carries the session id (the change log keeps one record per session, so History shows one line
 * per opening of the sheet), and a failed save keeps the text and tries again on the next edit or
 * Retry. Saves run one after another. A Task can't have a blank title, so a cleared title is never
 * saved: the notes still are, under the title that was stored, and leaving the field brings the
 * stored title back.
 */
export class TaskEditing<T> {
  title: string;
  notes: string;
  #savedTitle: string;
  #savedNotes: string;
  #cancelTimer: (() => void) | null = null;
  #queue: Promise<void> = Promise.resolve();

  constructor(
    private readonly deps: TaskEditingDeps<T>,
    readonly id: string,
    initial: { title: string; notes: string },
    readonly sessionId: string,
  ) {
    this.title = initial.title;
    this.notes = initial.notes;
    this.#savedTitle = initial.title;
    this.#savedNotes = initial.notes;
  }

  /** The title a save would send: the typed one, or the stored one while the field is blank. */
  get #titleToSave(): string {
    return this.title.trim() || this.#savedTitle;
  }

  /** Text that differs from what is stored and could be saved. */
  get unsaved(): boolean {
    return this.#titleToSave !== this.#savedTitle || this.notes !== this.#savedNotes;
  }

  /** The title field is empty, so the stored title is still the Task's. */
  get blankTitle(): boolean {
    return this.title.trim() === '';
  }

  /** The title changed (every keystroke). Returns the text to show, line breaks folded. */
  setTitle(text: string): string {
    this.title = oneLine(text);
    this.#schedule();
    return this.title;
  }

  /** The notes changed (every keystroke). */
  setNotes(text: string): void {
    this.notes = text;
    this.#schedule();
  }

  /** Leaving the title field: a blank title is put back as it was stored. Returns the title. */
  leaveTitle(): string {
    if (this.blankTitle) {
      this.title = this.#savedTitle;
      this.#schedule();
    }
    return this.title;
  }

  /** Takes text changed elsewhere (a repeating Task's Checklist reset on completion) as stored. */
  adopt(task: { title: string; notes: string }): void {
    this.#cancelTimer?.();
    this.#cancelTimer = null;
    this.title = this.#savedTitle = task.title;
    this.notes = this.#savedNotes = task.notes;
  }

  /** Saves what is pending now. True when nothing unsaved remains. */
  async flush(): Promise<boolean> {
    this.#cancelTimer?.();
    this.#cancelTimer = null;
    this.#save();
    await this.#queue;
    return !this.unsaved;
  }

  #schedule() {
    this.#cancelTimer?.();
    this.#cancelTimer = this.unsaved ? this.deps.setTimer(() => this.#save(), SAVE_PAUSE_MS) : null;
  }

  #save() {
    this.#queue = this.#queue.then(async () => {
      const title = this.#titleToSave;
      const notes = this.notes;
      if (title === this.#savedTitle && notes === this.#savedNotes) return;
      try {
        const task = await this.deps.update(this.id, title, notes, this.sessionId);
        this.#savedTitle = title;
        this.#savedNotes = notes;
        this.deps.onSaved(task);
      } catch (e) {
        this.deps.onError(e);
      }
    });
  }
}
