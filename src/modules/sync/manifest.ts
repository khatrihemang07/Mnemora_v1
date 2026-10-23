import GitMerge from '~icons/lucide/git-merge';
import RadioTower from '~icons/lucide/radio-tower';
import type { ModuleManifest } from '../../lib/router';
import { useSyncSwitch } from './switch.svelte';

export const syncManifest: ModuleManifest = {
  id: 'sync',
  name: 'Sync',
  description: 'Keep your devices in step, directly over Wi-Fi or Tailscale',
  icon: RadioTower,
  accentVar: '--hue-sync',
  // One address per page (Devices, Conflicts, Activity); the first is the Module's entry.
  routes: [
    { path: '/sync', component: () => import('./DevicesPage.svelte') },
    { path: '/sync/conflicts', component: () => import('./ConflictsPage.svelte') },
    { path: '/sync/conflicts/:id', component: () => import('./ComparePage.svelte') },
    { path: '/sync/activity', component: () => import('./ActivityPage.svelte') },
  ],
  // Beside Settings in the sidebar's bottom group, with no ⌘-digit: the digits keep matching content Modules.
  nav: { order: 90, shortcut: null, placement: 'bottom' },
  defaultEnabled: false,
  deviceSwitch: useSyncSwitch,
  settingsSection: {
    label: 'Sync',
    icon: RadioTower,
    component: () => import('./SyncSettings.svelte'),
  },
  overlay: () => import('./SyncOverlay.svelte'),
  paletteActions: [
    {
      id: 'conflicts',
      label: 'Open Sync conflicts',
      icon: GitMerge,
      run: (ctx) => ctx.navigate('/sync/conflicts'),
    },
  ],
};
