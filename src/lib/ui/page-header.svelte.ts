import type { Component, Snippet } from 'svelte';
import type { Environment } from '../ipc/bindings';

export interface PageHeaderEntry {
  title: string;
  /** A Module icon shown before the title, tinted with `hue`. */
  icon?: Component<{ class?: string }>;
  /** A Module hue token name, e.g. `--hue-todo`. */
  hue?: string;
  actions?: Snippet;
  /** Replaces the icon and title with the page's own heading (a renamable List title). */
  heading?: Snippet;
}

/** What the desktop top bar shows. Written by `PageHeader`, read by the shell's top bar. */
class PageHeaderState {
  current = $state.raw<PageHeaderEntry | null>(null);
  /**
   * A bar a page pins under its large title on a phone (Todo's view segments). Set by the page's
   * owner, drawn by `PageHeader` where the title is, so it follows whichever view is showing.
   */
  phoneBar = $state.raw<Snippet | null>(null);
  /** The build's Environment, set once by the shell; the phone title shows a SANDBOX badge from it. */
  env = $state<Environment | null>(null);

  /** Pin `bar` under the phone title until the returned function runs. */
  setPhoneBar(bar: Snippet): () => void {
    this.phoneBar = bar;
    return () => {
      if (this.phoneBar === bar) this.phoneBar = null;
    };
  }

  /** Claim the bar; the returned function releases it unless a newer page already took over. */
  set(entry: PageHeaderEntry): () => void {
    this.current = entry;
    return () => {
      if (this.current === entry) this.current = null;
    };
  }
}

export const pageHeader = new PageHeaderState();
