import NotebookPen from '~icons/lucide/notebook-pen';
import Search from '~icons/lucide/search';
import { shortcutLabel } from '../../lib/platform';
import type { ModuleManifest } from '../../lib/router';
import { requestSearch } from './search.svelte';

export const journalManifest: ModuleManifest = {
  id: 'journal',
  name: 'Journal',
  icon: NotebookPen,
  accentVar: '--hue-journal',
  routes: [{ path: '/journal', component: () => import('./JournalPage.svelte') }],
  nav: { order: 20, shortcut: 2 },
  // ⌘N opens the Journal; the composer takes focus on open.
  create: (ctx) => ctx.navigate('/journal'),
  paletteActions: [
    {
      id: 'search',
      label: 'Search Journal',
      icon: Search,
      shortcut: shortcutLabel('F'),
      run: (ctx) => {
        requestSearch('');
        ctx.navigate('/journal');
      },
    },
    {
      // Shown once something is typed: ⌘K, "river", Enter searches the Journal for it.
      id: 'search-for',
      label: 'Search Journal for…',
      icon: Search,
      queryLabel: (query) => `Search Journal for “${query}”`,
      run: (ctx, query) => {
        requestSearch(query);
        ctx.navigate('/journal');
      },
    },
  ],
};
