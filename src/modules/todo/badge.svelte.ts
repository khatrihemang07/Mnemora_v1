import { createQuery } from '@tanstack/svelte-query';
import { clock } from '../../lib/time/clock.svelte';
import { type Badge, badgeOf } from './counts';
import { countsQuery } from './queries';

/**
 * Todo's badge for the sidebar and the phone tab bar: due today plus Overdue, red while anything
 * is Overdue. Call during component setup. It keeps the shared clock running, so the count rolls
 * over at midnight even when no Todo page is open.
 */
export function useTodoBadge(): { readonly badge: Badge | null } {
  $effect(() => clock.subscribe());
  const counts = createQuery(() => countsQuery());
  const badge = $derived(badgeOf(counts.data));
  return {
    get badge() {
      return badge;
    },
  };
}
