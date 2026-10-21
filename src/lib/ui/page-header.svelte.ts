import type { Component, Snippet } from 'svelte';
import type { Environment } from '../ipc/bindings';

export interface PageHeaderEntry {
  title: string;
  /** A Module icon shown before the title, tinted with `hue`. */
  icon?: Component<{ class?: string }>;
  /** A Module hue token name, e.g. `--hue-todo`. */
  hue?: string;
  actions?: Snippet;
}

/** What the desktop top bar shows. Written by `PageHeader`, read by the shell's top bar. */
class PageHeaderState {
  current = $state.raw<PageHeaderEntry | null>(null);
  /** The build's Environment, set once by the shell; the phone title shows a SANDBOX badge from it. */
  env = $state<Environment | null>(null);

  /** Claim the bar; the returned function releases it unless a newer page already took over. */
  set(entry: PageHeaderEntry): () => void {
    this.current = entry;
    return () => {
      if (this.current === entry) this.current = null;
    };
  }
}

export const pageHeader = new PageHeaderState();
