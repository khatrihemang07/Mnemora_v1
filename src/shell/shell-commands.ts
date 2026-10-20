import { commands } from '../lib/ipc/bindings';
import { unwrap } from '../lib/ipc/unwrap';
import { notify, notifyError } from '../lib/ui/toast';
import { viewport } from '../lib/viewport.svelte';
import { buildPaletteItems } from './palette-items';
import { modules } from './registry';
import { router } from './router';
import { shell } from './shell-state.svelte';
import type { ShortcutAction } from './shortcuts';

/** Palette theme commands are explicit, so they apply and save at once (no draft). */
async function setTheme(theme: string) {
  try {
    await unwrap(commands.settingsSave([{ key: 'core.theme', value: theme }]));
    notify('Theme updated');
  } catch (e) {
    notifyError(e instanceof Error ? e.message : 'Could not change theme');
  }
}

export const paletteItems = buildPaletteItems({
  setTheme: (t) => void setTheme(t),
  toggleSidebar: () => shell.toggleSidebar(),
});

/** Carries out a resolved keyboard shortcut. */
export function runShortcut(a: ShortcutAction) {
  switch (a.type) {
    case 'palette':
      shell.paletteOpen = !shell.paletteOpen;
      break;
    case 'settings':
      void router.navigate('/settings');
      break;
    case 'sidebar':
      if (!viewport.isPhone) shell.toggleSidebar();
      break;
    case 'create':
      notify('Nothing to create yet');
      break;
    case 'module': {
      const route = modules.find((x) => x.nav.shortcut === a.digit)?.routes[0];
      if (route) void router.navigate(route.path);
      break;
    }
  }
}
