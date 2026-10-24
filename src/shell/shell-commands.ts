import { commands } from '../lib/ipc/bindings';
import { unwrap } from '../lib/ipc/unwrap';
import type { ModuleManifest } from '../lib/router';
import { notify, notifyError } from '../lib/ui/toast';
import { viewport } from '../lib/viewport.svelte';
import { createTarget } from './enabled-modules';
import { moduleContext } from './module-context';
import { buildPaletteItems } from './palette-items';
import { router } from './router';
import { shell } from './shell-state.svelte';
import type { ShortcutAction } from './shortcuts';

/** Palette theme commands are explicit, so they apply and save at once (no draft). */
async function setTheme(theme: string) {
  try {
    await unwrap(commands.settingsSave([{ key: 'core.theme', value: theme }]));
  } catch (e) {
    notifyError(e instanceof Error ? e.message : 'Could not change theme');
  }
}

/** ⌘K items for the given enabled Modules. */
export const paletteItemsFor = (enabled: ModuleManifest[]) =>
  buildPaletteItems({
    setTheme: (t) => void setTheme(t),
    toggleSidebar: () => shell.toggleSidebar(),
    enabled,
  });

/** Carries out a resolved keyboard shortcut; `enabled` are the Modules currently on. */
export function runShortcut(a: ShortcutAction, enabled: ModuleManifest[]) {
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
    case 'create': {
      const target = createTarget(enabled);
      if (target?.create) target.create(moduleContext);
      else notify('Nothing to create yet');
      break;
    }
    case 'module': {
      const route = enabled.find((x) => x.nav.shortcut === a.digit)?.routes[0];
      if (route) void router.navigate(route.path);
      break;
    }
  }
}
