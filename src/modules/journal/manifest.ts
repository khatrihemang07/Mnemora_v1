import CalendarDays from '~icons/lucide/calendar-days';
import NotebookPen from '~icons/lucide/notebook-pen';
import PenLine from '~icons/lucide/pen-line';
import Search from '~icons/lucide/search';
import { shortcutLabel } from '../../lib/platform';
import type { ModuleContext, ModuleManifest } from '../../lib/router';
import { requestCompose, requestGoToDate } from './launcher.svelte';
import { requestSearch } from './search.svelte';

/** ⌘N, "New journal entry" and the Home card's Write: Today, composer focused, from anywhere. */
function compose(ctx: ModuleContext) {
  requestCompose();
  ctx.navigate('/journal');
}

export const journalManifest: ModuleManifest = {
  id: 'journal',
  name: 'Journal',
  icon: NotebookPen,
  accentVar: '--hue-journal',
  routes: [{ path: '/journal', component: () => import('./JournalPage.svelte') }],
  nav: { order: 20, shortcut: 2 },
  homeCard: () => import('./HomeCard.svelte'),
  settingsSection: {
    label: 'Journal',
    icon: NotebookPen,
    component: () => import('./JournalSettings.svelte'),
  },
  create: compose,
  paletteActions: [
    {
      id: 'new',
      label: 'New journal entry',
      icon: PenLine,
      shortcut: shortcutLabel('N'),
      run: compose,
    },
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
      id: 'goto',
      label: 'Go to date…',
      icon: CalendarDays,
      shortcut: shortcutLabel('G'),
      run: (ctx) => {
        requestGoToDate();
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
