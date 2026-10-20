import SquareCheckBig from '~icons/lucide/square-check-big';
import type { ModuleManifest } from '../../lib/router';

export const todoManifest: ModuleManifest = {
  id: 'todo',
  name: 'Todo',
  icon: SquareCheckBig,
  accentVar: '--hue-todo',
  routes: [{ path: '/todo', component: () => import('./TodoPage.svelte') }],
  nav: { order: 30, shortcut: 3 },
};
