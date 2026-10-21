import { createQuery } from '@tanstack/svelte-query';
import { readCore } from '../../lib/settings/core';
import { settingsQuery } from '../../lib/settings/queries';
import type { TimeFormat } from '../../lib/time/format';
import type { WeekStart } from '../../lib/ui/calendar';

/** The General Settings Todo's date and time displays follow, kept where every row can read them. */
class TodoPrefs {
  weekStart = $state<WeekStart>('mon');
  timeFormat = $state<TimeFormat>('24h');
  /** The font of a Task's notes: the Journal's Writing font Setting, so there is one choice. */
  notesFont = $state<'serif' | 'sans'>('serif');
}

export const todoPrefs = new TodoPrefs();

/** Call once from Todo's page: follows the Settings while Todo is open. */
export function followCoreSettings() {
  const settings = createQuery(() => settingsQuery);
  $effect(() => {
    const core = readCore(settings.data?.values);
    todoPrefs.weekStart = core.weekStart;
    todoPrefs.timeFormat = core.timeFormat;
    // Read as plain Setting data: Todo never imports the Journal.
    todoPrefs.notesFont = settings.data?.values['journal.font'] === 'sans' ? 'sans' : 'serif';
  });
}
