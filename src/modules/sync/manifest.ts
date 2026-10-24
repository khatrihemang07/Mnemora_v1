import GitMerge from '~icons/lucide/git-merge';
import Plus from '~icons/lucide/plus';
import RadioTower from '~icons/lucide/radio-tower';
import RefreshCw from '~icons/lucide/refresh-cw';
import { commands } from '../../lib/ipc/bindings';
import { queryClient } from '../../lib/ipc/query-client';
import type { ModuleManifest } from '../../lib/router';
import { notify } from '../../lib/ui/toast';
import { paletteHint } from './attention';
import { useSyncBadge } from './badge.svelte';
import { syncAttention } from './hint.svelte';
import { pairing } from './pairing.svelte';
import { syncQuery } from './queries';
import { useSyncSwitch } from './switch.svelte';
import { runSyncNow } from './sync-now';

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
  nav: { order: 90, placement: 'bottom' },
  bareKeys: [{ key: 's', run: (ctx) => ctx.navigate('/sync') }],
  defaultEnabled: false,
  deviceSwitch: useSyncSwitch,
  // The conflicts that need you, an amber "!" while paused: the desktop sidebar only (B7, B8).
  useBadge: useSyncBadge,
  phoneBadge: false,
  // Only while something needs you (B10).
  homeCard: () => import('./HomeCard.svelte'),
  settingsSection: {
    label: 'Sync',
    icon: RadioTower,
    component: () => import('./SyncSettings.svelte'),
  },
  overlay: () => import('./SyncOverlay.svelte'),
  paletteActions: [
    {
      id: 'now',
      label: 'Sync now',
      icon: RefreshCw,
      run: () =>
        void runSyncNow(
          async () => (await queryClient.fetchQuery(syncQuery)).paired_devices,
          () => commands.syncNow(),
          notify,
        ),
    },
    {
      id: 'pair',
      label: 'Pair a device',
      icon: Plus,
      run: (ctx) => {
        ctx.navigate('/sync');
        void pairing.start('show');
      },
    },
    {
      id: 'conflicts',
      label: 'Open Sync conflicts',
      icon: GitMerge,
      hint: () => paletteHint(syncAttention.current),
      run: (ctx) => ctx.navigate('/sync/conflicts'),
    },
  ],
};
