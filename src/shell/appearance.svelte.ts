import { createQuery } from '@tanstack/svelte-query';
import { readCore } from '../lib/settings/core';
import { settingsQuery } from '../lib/settings/queries';
import { settingsDraft } from '../lib/settings/store.svelte';
import { applyAppearance, watchSystemTheme } from '../lib/theme/apply';

/** Settings to <html>: draft edits preview live; Reduce motion waits for Save. Call from a Component. */
export function useAppearance() {
  const settings = createQuery(() => settingsQuery);

  $effect(() => {
    if (settings.data) settingsDraft.rebase(settings.data.values);
  });
  $effect(() => {
    if (!settings.data) return; // keep the pre-paint attributes until real values arrive
    const saved = readCore(settings.data.values);
    const apply = () =>
      applyAppearance(document.documentElement, {
        ...readCore(settingsDraft.merged()),
        reduceMotion: saved.reduceMotion,
      });
    apply();
    if (readCore(settingsDraft.merged()).theme === 'system') return watchSystemTheme(apply);
  });
}
