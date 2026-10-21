import { createQuery } from '@tanstack/svelte-query';
import { readCore } from '../../lib/settings/core';
import { settingsQuery } from '../../lib/settings/queries';
import type { TimeFormat } from '../../lib/time/format';
import type { WeekStart } from '../../lib/ui/calendar';

/** The General Settings Todo's date and time displays follow, kept where every row can read them. */
class TodoPrefs {
  weekStart = $state<WeekStart>('mon');
  timeFormat = $state<TimeFormat>('24h');
}

export const todoPrefs = new TodoPrefs();

/** Call once from Todo's page: follows the Settings while Todo is open. */
export function followCoreSettings() {
  const settings = createQuery(() => settingsQuery);
  $effect(() => {
    const core = readCore(settings.data?.values);
    todoPrefs.weekStart = core.weekStart;
    todoPrefs.timeFormat = core.timeFormat;
  });
}
