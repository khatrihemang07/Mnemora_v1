<!--
  WhenHost: the When picker wired to Todo: today and the clock, the Week start and time format
  Settings, Rust's parser for the "Type a date" field, and the dots, loaded for the weeks on show (leaving out `exclude`, the Task being
  dated). The row hover icon, the row menu and Move all use it, so the picker works the same
  everywhere. `onchange` gets the next date and time.
-->
<script lang="ts">
  import { createQuery } from '@tanstack/svelte-query';
  import WhenPicker, { type WhenValue } from '../../lib/ui/WhenPicker.svelte';
  import { todoClock } from './clock.svelte';
  import { todoPrefs } from './prefs.svelte';
  import { dayCountsQuery } from './queries';
  import { typedWhen } from './when-typed';

  interface Props {
    open: boolean;
    anchor: HTMLElement | null;
    value?: WhenValue;
    /** The Task being dated, so its own date does not add a dot. */
    exclude?: string | null;
    title?: string;
    /** False for Move all: a date only, and no way to clear it. */
    dateOnly?: boolean;
    restoreFocus?: () => void;
    onchange: (next: WhenValue) => void;
  }
  let {
    open = $bindable(),
    anchor,
    value = { date: null, time: null, repeat: null },
    exclude = null,
    title = 'When',
    dateOnly = false,
    restoreFocus,
    onchange,
  }: Props = $props();

  let range = $state.raw<{ from: string; to: string } | null>(null);
  const counts = createQuery(() => ({
    ...dayCountsQuery(range?.from ?? '', range?.to ?? '', exclude),
    enabled: open && range !== null,
  }));
  const dots = $derived(new Map((counts.data ?? []).map((d) => [d.date, d.count])));
</script>

<WhenPicker
  bind:open
  {anchor}
  {value}
  {title}
  today={todoClock.today}
  now={todoClock.hm}
  weekStart={todoPrefs.weekStart}
  timeFormat={todoPrefs.timeFormat}
  parse={typedWhen}
  counts={dots}
  showTime={!dateOnly}
  showRepeat={!dateOnly}
  allowClear={!dateOnly}
  {restoreFocus}
  {onchange}
  onrange={(from, to) => (range = { from, to })}
/>
