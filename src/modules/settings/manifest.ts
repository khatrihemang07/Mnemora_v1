import Settings from '~icons/lucide/settings';
import type { ModuleManifest } from '../../lib/router';
import { settingsNav } from './settings-nav.svelte';

export const settingsManifest: ModuleManifest = {
  id: 'settings',
  name: 'Settings',
  icon: Settings,
  accentVar: '--hue-settings',
  routes: [{ path: '/settings', component: () => import('./SettingsPage.svelte') }],
  nav: { order: 100, placement: 'bottom' },
  required: true,
  // ? opens Settings on Keyboard shortcuts (desktop only; the section is skipped on a phone).
  bareKeys: [
    {
      key: '?',
      run: (ctx) => {
        settingsNav.section = 'shortcuts';
        ctx.navigate('/settings');
      },
    },
  ],
};
