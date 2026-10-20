import { createQuery } from '@tanstack/svelte-query';
import { readCore } from '../lib/settings/core';
import { settingsQuery } from '../lib/settings/queries';
import { enabledModules } from './enabled-modules';
import { modules } from './registry';

/**
 * The Modules currently switched on, from saved Settings (a pending draft doesn't count).
 * Call during component setup, like any query.
 */
export function useEnabledModules() {
  const settings = createQuery(() => settingsQuery);
  const list = $derived(enabledModules(modules, readCore(settings.data?.values).disabledModules));
  return {
    get list() {
      return list;
    },
  };
}
