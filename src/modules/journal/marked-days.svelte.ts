import { createQuery } from '@tanstack/svelte-query';
import { parseYmd, shiftMonth } from '../../lib/ui/calendar';
import { monthDaysQuery } from './queries';

/**
 * The days with Entries around a picker's month, for its coral dots: the month on show and the two
 * beside it, so the grey days at the edges get theirs too. Loads only while the picker is open.
 * Call during component setup.
 */
export function useMarkedDays(state: () => { month: string; open: boolean }) {
  const around = $derived([-1, 0, 1].map((d) => parseYmd(shiftMonth(state().month, d))));
  const queries = [0, 1, 2].map((i) =>
    createQuery(() => ({
      ...monthDaysQuery(around[i]?.[0] ?? 1970, around[i]?.[1] ?? 1),
      enabled: state().open,
    })),
  );
  const marked = $derived(new Set(queries.flatMap((q) => q.data ?? [])));
  return {
    get marked() {
      return marked;
    },
  };
}
