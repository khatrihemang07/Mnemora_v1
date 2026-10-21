import type { LocalNow } from '../../lib/ipc/bindings';
import { clock } from '../../lib/time/clock.svelte';
import { localDate, localNow, localTime } from './dates';

/**
 * Todo's view of the shared clock: the device's today and the minute, as strings, so what reads
 * them updates only when the day (midnight) or the minute changes. The shared clock ticks while
 * something is subscribed (Todo's page does), so nothing polls.
 */
class TodoClock {
  today = $derived(localDate(clock.now));
  /** `HH:MM`. */
  hm = $derived(localTime(clock.now));

  /** The clock as date-dependent commands want it, read fresh at the moment of the call. */
  local(): LocalNow {
    return localNow(new Date());
  }
}

export const todoClock = new TodoClock();
