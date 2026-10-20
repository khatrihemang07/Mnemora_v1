<!--
  SearchRangeChip: the "Any date ▾" chip beside the search field. It opens a Menu of presets (Past
  week, month, year) and "Custom range…", which opens the DatePicker in range mode. An active range
  shows on the chip (tinted) with a × beside it to clear. The filter is by Page date.
-->
<script lang="ts">
  import CalendarRange from '~icons/lucide/calendar-range';
  import X from '~icons/lucide/x';
  import { EMPTY_RANGE, type WeekStart } from '../../lib/ui/calendar';
  import Chip from '../../lib/ui/Chip.svelte';
  import DatePicker from '../../lib/ui/DatePicker.svelte';
  import IconButton from '../../lib/ui/IconButton.svelte';
  import Menu, { type MenuItem } from '../../lib/ui/Menu.svelte';
  import { PRESETS, presetRange, rangeLabel, type SearchRange } from './search-state';

  interface Props {
    range: SearchRange | null;
    today: string;
    weekStart: WeekStart;
    onchange: (range: SearchRange | null) => void;
    /** Where focus goes once a choice is made: back into the search field. */
    onrestorefocus: () => void;
  }
  let { range, today, weekStart, onchange, onrestorefocus }: Props = $props();

  let menuOpen = $state(false);
  let pickerOpen = $state(false);
  let chip = $state<HTMLButtonElement | null>(null);

  const label = $derived(rangeLabel(range, today));
  const items: MenuItem[] = $derived([
    ...PRESETS.map((p) => ({
      label: p.label,
      onselect: () => {
        onchange(presetRange(p.id, today));
        onrestorefocus();
      },
    })),
    {
      label: 'Custom range…',
      icon: CalendarRange,
      separatorBefore: true,
      onselect: () => {
        pickerOpen = true;
      },
    },
  ]);
</script>

<div class="flex shrink-0 items-center gap-1">
  <Chip
    bind:ref={chip}
    icon={CalendarRange}
    tone={range ? 'warm' : 'plain'}
    hue="--hue-journal"
    aria-label="Date filter: {label}. Change"
    aria-haspopup="menu"
    aria-expanded={menuOpen}
    onclick={() => (menuOpen = !menuOpen)}
  >
    {label}
  </Chip>
  {#if range}
    <IconButton
      icon={X}
      label="Clear date filter"
      size="xs"
      variant="quiet"
      onclick={() => {
        onchange(null);
        onrestorefocus();
      }}
    />
  {/if}
</div>

<Menu bind:open={menuOpen} anchor={chip} {items} label="Date filter" />
<DatePicker
  bind:open={pickerOpen}
  anchor={chip}
  title="Search between dates"
  mode="range"
  {today}
  {weekStart}
  range={range ? { from: range.from, to: range.to } : EMPTY_RANGE}
  hue="--hue-journal"
  onrange={(r) => {
    if (r.from !== null && r.to !== null) onchange({ from: r.from, to: r.to, preset: null });
  }}
  restoreFocus={onrestorefocus}
/>
