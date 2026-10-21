import SquareCheckBig from '~icons/lucide/square-check-big';
import type { ModuleManifest } from '../../lib/router';
import { quickAdd } from './quick-add.svelte';

export const todoManifest: ModuleManifest = {
  id: 'todo',
  name: 'Todo',
  icon: SquareCheckBig,
  accentVar: '--hue-todo',
  routes: [{ path: '/todo', component: () => import('./TodoPage.svelte') }],
  nav: { order: 30, shortcut: 3 },
  // Q adds a Task from anywhere (ADR 0011): in place inside Todo, in a floating box elsewhere.
  bareKeys: [{ key: 'q', run: (ctx) => quickAdd.request(ctx.isActive('/todo')) }],
  overlay: () => import('./QuickAddHost.svelte'),
  navTree: {
    tree: () => import('./TodoTree.svelte'),
    flyout: () => import('./TodoFlyout.svelte'),
  },
};
