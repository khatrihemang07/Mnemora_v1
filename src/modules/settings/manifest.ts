import Settings from '~icons/lucide/settings';
import type { ModuleManifest } from '../../lib/router';

export const settingsManifest: ModuleManifest = {
  id: 'settings',
  name: 'Settings',
  icon: Settings,
  accentVar: '--hue-settings',
  routes: [{ path: '/settings', component: () => import('./SettingsPage.svelte') }],
  nav: { order: 100, shortcut: null, placement: 'bottom' },
};
