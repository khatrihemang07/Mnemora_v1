import type { Snippet } from 'svelte';

export interface PageHeaderEntry {
  title: string;
  actions?: Snippet;
}

/** What the desktop top bar shows. Written by `PageHeader`, read by the shell's top bar. */
class PageHeaderState {
  current = $state.raw<PageHeaderEntry | null>(null);

  /** Claim the bar; the returned function releases it unless a newer page already took over. */
  set(entry: PageHeaderEntry): () => void {
    this.current = entry;
    return () => {
      if (this.current === entry) this.current = null;
    };
  }
}

export const pageHeader = new PageHeaderState();
