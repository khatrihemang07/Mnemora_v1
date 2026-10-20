import NotebookPen from '~icons/lucide/notebook-pen';
import type { ModuleManifest } from '../../lib/router';

export const journalManifest: ModuleManifest = {
  id: 'journal',
  name: 'Journal',
  icon: NotebookPen,
  accentVar: '--hue-journal',
  routes: [{ path: '/journal', component: () => import('./JournalPage.svelte') }],
  nav: { order: 20, shortcut: 2 },
  // ⌘N opens the Journal; the composer takes focus on open.
  create: (ctx) => ctx.navigate('/journal'),
};
