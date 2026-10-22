import { createQuery } from '@tanstack/svelte-query';
import { readCore } from '../lib/settings/core';
import { settingsQuery } from '../lib/settings/queries';
import { enabledModules } from './enabled-modules';
import { moduleContext } from './module-context';
import { modules } from './registry';

/**
 * The Modules currently switched on, from saved Settings (a pending draft doesn't count) and, for a
 * Module whose switch is a Device setting (Sync), from this device. Call during component setup,
 * like any query.
 */
export function useEnabledModules() {
  const settings = createQuery(() => settingsQuery);
  const deviceSwitches = modules.flatMap((m) =>
    m.deviceSwitch ? [{ id: m.id, sw: m.deviceSwitch(moduleContext) }] : [],
  );
  const list = $derived.by(() => {
    const core = readCore(settings.data?.values);
    const deviceOn = deviceSwitches.filter((d) => d.sw.on).map((d) => d.id);
    return enabledModules(modules, core.disabledModules, [
      ...core.enabledOptionalModules,
      ...deviceOn,
    ]);
  });
  // A default-off Module reads as off until the device file is read; its URL must not be sent Home meanwhile.
  const ready = $derived(deviceSwitches.every((d) => d.sw.ready));
  return {
    get list() {
      return list;
    },
    get ready() {
      return ready;
    },
  };
}
