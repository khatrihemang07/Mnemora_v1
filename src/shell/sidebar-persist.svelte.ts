import { createQuery } from '@tanstack/svelte-query';
import { commands } from '../lib/ipc/bindings';
import { deviceQuery } from '../lib/settings/queries';
import { PERSIST_DEBOUNCE_MS } from '../lib/timing';
import { shell } from './shell-state.svelte';

/** Restores the sidebar layout and tree fold once from Device settings, then persists changes (debounced). */
export function useSidebarPersistence() {
  const device = createQuery(() => deviceQuery);

  $effect(() => {
    if (shell.layoutReady || !(device.isSuccess || device.isError)) return;
    const s = device.data?.sidebar;
    shell.layout = { width: s?.width ?? shell.layout.width, rail: s?.rail ?? false };
    shell.treeOpen = s?.todo_open ?? false;
    shell.treeUnset = s?.todo_open == null;
    shell.layoutReady = true;
  });
  $effect(() => {
    if (!shell.layoutReady || shell.treeUnset) return;
    const open = shell.treeOpen;
    const t = setTimeout(() => void commands.deviceSetTodoOpen(open), PERSIST_DEBOUNCE_MS);
    return () => clearTimeout(t);
  });
  $effect(() => {
    if (!shell.layoutReady) return;
    const { width, rail } = shell.layout;
    const t = setTimeout(() => void commands.deviceSetSidebar(width, rail), PERSIST_DEBOUNCE_MS);
    return () => clearTimeout(t);
  });
}
