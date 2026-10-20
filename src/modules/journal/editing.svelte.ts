import { getContext, setContext } from 'svelte';
import { LeaveGuard } from '../../lib/leave-guard.svelte';
import type { EditorCommand } from '../../lib/ui/markdown/shortcuts';
import type { SlashValues } from '../../lib/ui/markdown/slash';

/** What a focused editor offers the phone KeyboardBar. */
export interface ActiveEditor {
  run: (command: EditorCommand) => boolean;
}

/** Asks before leaving the Journal with text that failed to save. */
export const journalLeaveGuard = new LeaveGuard();

/**
 * Journal-wide editing state shared by the feed, the Entries and the composer through context: which
 * Entry is open (the rest dim), which editor the phone bar drives, and the Entry the composer just
 * finished (it slides into its Page).
 */
export class Editing {
  /** The Entry open for inline editing. */
  id = $state<string | null>(null);
  /** The editor that has focus, if any. */
  active = $state.raw<ActiveEditor | null>(null);
  /** An Entry the composer just finished; its row plays the arrival once. */
  arrived = $state<string | null>(null);
  /** Date and time for the slash menu in the General Settings formats. */
  slashValues: () => SlashValues = () => ({ date: '', time: '' });
  spellcheck = $state(true);
  /** Set by the open editor: saves and closes it, false when text is still unsaved. */
  finisher: (() => Promise<boolean>) | null = null;

  /** Opens `id`, first finishing any other open Entry; stays put if that one can't be saved. */
  async open(id: string): Promise<void> {
    if (this.id === id) return;
    if (this.id !== null && this.finisher && !(await this.finisher())) return;
    this.id = id;
  }

  close(): void {
    this.id = null;
    this.finisher = null;
  }
}

const KEY = Symbol('journal-editing');
export const provideEditing = (e: Editing) => setContext(KEY, e);
export const useEditing = () => getContext<Editing>(KEY);
