import Plus from '~icons/lucide/plus';
import Search from '~icons/lucide/search';
import SquareCheckBig from '~icons/lucide/square-check-big';
import { shortcutLabel } from '../../lib/platform';
import type { ModuleAction, ModuleManifest } from '../../lib/router';
import { useTodoBadge } from './badge.svelte';
import { todoNav } from './nav.svelte';
import { addTaskFromPalette, previewAddTask } from './palette-add';
import { quickAdd } from './quick-add.svelte';
import { requestSearch } from './search.svelte';
import type { GoTarget } from './task-keymap';
import { VIEW_ICONS } from './view-icons';
import { SMART_VIEWS, smartView } from './views';

/** ⌘K's "Go to Today" and the rest: the G chords' twins, from anywhere in the app. */
const GO_CHORDS: Record<GoTarget, string> = {
  today: 'G T',
  upcoming: 'G U',
  inbox: 'G I',
  all: 'G A',
  done: 'G D',
};

const goActions: ModuleAction[] = SMART_VIEWS.map((v) => ({
  id: `go-${v.id}`,
  label: `Go to ${v.label}`,
  icon: VIEW_ICONS[v.id],
  shortcut: GO_CHORDS[v.id],
  run: (ctx) => {
    todoNav.go(smartView(v.id));
    ctx.navigate('/todo');
  },
}));

export const todoManifest: ModuleManifest = {
  id: 'todo',
  name: 'Todo',
  icon: SquareCheckBig,
  accentVar: '--hue-todo',
  routes: [{ path: '/todo', component: () => import('./TodoPage.svelte') }],
  nav: { order: 30, shortcut: 3 },
  useBadge: useTodoBadge,
  homeCard: () => import('./HomeCard.svelte'),
  // Q adds a Task from anywhere (ADR 0011): in place inside Todo, in a floating box elsewhere.
  bareKeys: [{ key: 'q', run: (ctx) => quickAdd.request(ctx.isActive('/todo')) }],
  overlay: () => import('./TodoOverlay.svelte'),
  paletteActions: [
    ...goActions,
    {
      // Shown once something is typed: ⌘K, "Call mum tomorrow 6pm p2", Enter adds it, read like
      // the capture box reads it, and the line beside it says how.
      id: 'add',
      label: 'Add task',
      icon: Plus,
      queryLabel: (query) => `Add task: “${query}”`,
      queryPreview: previewAddTask,
      run: (_ctx, query) => void addTaskFromPalette(query),
    },
    {
      id: 'search',
      label: 'Search tasks',
      icon: Search,
      shortcut: shortcutLabel('F'),
      run: (ctx) => {
        requestSearch('');
        ctx.navigate('/todo');
      },
    },
    {
      // Shown once something is typed: ⌘K, "passport", Enter searches Tasks for it.
      id: 'search-for',
      label: 'Search tasks for…',
      icon: Search,
      queryLabel: (query) => `Search tasks for “${query}”`,
      run: (ctx, query) => {
        requestSearch(query);
        ctx.navigate('/todo');
      },
    },
  ],
  settingsSection: {
    label: 'Todo',
    icon: SquareCheckBig,
    component: () => import('./TodoSettings.svelte'),
  },
  navTree: {
    tree: () => import('./TodoTree.svelte'),
    flyout: () => import('./TodoFlyout.svelte'),
  },
};
