<!--
  Calendar: a month grid for picking a date. Dots mark days (`marked`), a ring marks today, `selected`
  is filled and a `range` tints its days; `disabled` greys days out. The title opens a month and year
  picker; ‹ › step months; an optional Today button jumps back. Keyboard: arrows move within the grid
  (PageUp/PageDown change month, Home/End the week's ends), Enter or Space picks. The week starts on
  `weekStart`. Dates are `YYYY-MM-DD`; `month` is `YYYY-MM` and bindable. `hue` is a Module hue token
  name (e.g. `--hue-journal`) for the today ring and dots; the brand accent by default.
-->
<script lang="ts">
  import { tick } from 'svelte';
  import ChevronDown from '~icons/lucide/chevron-down';
  import ChevronLeft from '~icons/lucide/chevron-left';
  import ChevronRight from '~icons/lucide/chevron-right';
  import Button from './Button.svelte';
  import {
    type DateRange,
    inRange,
    monthGrid,
    moveFocus,
    parseYmd,
    shiftMonth,
    type WeekStart,
    weekdayLabels,
  } from './calendar';
  import IconButton from './IconButton.svelte';

  interface Props {
    /** The month shown, `YYYY-MM`. */
    month: string;
    today: string;
    selected?: string | null;
    /** Days to mark with a dot. */
    marked?: ReadonlySet<string>;
    weekStart: WeekStart;
    disabled?: (date: string) => boolean;
    range?: DateRange;
    showToday?: boolean;
    hue?: string;
    onpick: (date: string) => void;
    /** Called when the Today button is used, after the view has moved to today's month. */
    ontoday?: () => void;
    class?: string;
  }
  let {
    month = $bindable(),
    today,
    selected = null,
    marked,
    weekStart,
    disabled,
    range,
    showToday = false,
    hue,
    onpick,
    ontoday,
    class: cls = '',
  }: Props = $props();

  const MONTHS = Array.from({ length: 12 }, (_, i) =>
    new Intl.DateTimeFormat('en-GB', { month: 'long', timeZone: 'UTC' }).format(
      new Date(Date.UTC(2000, i, 1)),
    ),
  );
  const long = new Intl.DateTimeFormat('en-GB', {
    weekday: 'long',
    day: 'numeric',
    month: 'long',
    year: 'numeric',
    timeZone: 'UTC',
  });

  let view = $state<'days' | 'months'>('days');
  let pickerYear = $state(0);
  let focusDate = $state<string | null>(null);
  let grid = $state<HTMLDivElement | null>(null);

  const year = $derived(parseYmd(month)[0]);
  const monthIndex = $derived(parseYmd(month)[1] - 1);
  const days = $derived(monthGrid(month, weekStart));
  const weeks = $derived(Array.from({ length: 6 }, (_, i) => days.slice(i * 7, i * 7 + 7)));
  // The one day in the tab order: the last keyboard position, else the selection, else today.
  const tabDate = $derived(
    focusDate?.startsWith(month)
      ? focusDate
      : selected?.startsWith(month)
        ? selected
        : today.startsWith(month)
          ? today
          : `${month}-01`,
  );
  const isDisabled = (date: string) => disabled?.(date) ?? false;

  function openMonths() {
    pickerYear = year;
    view = 'months';
  }

  function chooseMonth(index: number) {
    month = `${String(pickerYear).padStart(4, '0')}-${String(index + 1).padStart(2, '0')}`;
    focusDate = null;
    view = 'days';
  }

  function pick(date: string) {
    if (isDisabled(date)) return;
    // A grey day from the next or previous month brings that month into view.
    if (!date.startsWith(month)) month = date.slice(0, 7);
    onpick(date);
  }

  async function onkeydown(e: KeyboardEvent) {
    if (e.altKey || e.ctrlKey || e.metaKey) return;
    const next = moveFocus(tabDate, e.key, weekStart);
    if (next === null) return;
    e.preventDefault();
    focusDate = next;
    if (!next.startsWith(month)) month = next.slice(0, 7);
    await tick();
    grid?.querySelector<HTMLElement>(`[data-date="${next}"]`)?.focus();
  }

  function goToday() {
    month = today.slice(0, 7);
    focusDate = today;
    view = 'days';
    ontoday?.();
  }
</script>

<div class="select-none {cls}" style={hue ? `--cal-hue: var(${hue})` : undefined}>
  <div class="mb-2 flex items-center gap-1">
    {#if view === 'days'}
      <button
        type="button"
        class="inline-flex flex-1 cursor-pointer items-center gap-1 rounded-sm px-1.5 py-1 text-left font-display text-[1.05rem] text-ink hover:bg-hover"
        style="font-weight: var(--display-weight)"
        aria-label="Choose month and year, now {MONTHS[monthIndex]} {year}"
        onclick={openMonths}
      >
        {MONTHS[monthIndex]}
        {year}
        <ChevronDown class="size-3.5 text-faint" />
      </button>
      <IconButton
        icon={ChevronLeft}
        label="Previous month"
        size="sm"
        onclick={() => (month = shiftMonth(month, -1))}
      />
      <IconButton
        icon={ChevronRight}
        label="Next month"
        size="sm"
        onclick={() => (month = shiftMonth(month, 1))}
      />
    {:else}
      <button
        type="button"
        class="inline-flex flex-1 cursor-pointer items-center gap-1 rounded-sm px-1.5 py-1 text-left font-display text-[1.05rem] text-ink hover:bg-hover"
        style="font-weight: var(--display-weight)"
        aria-label="Back to the days of {MONTHS[monthIndex]} {year}"
        onclick={() => (view = 'days')}
      >
        {pickerYear}
      </button>
      <IconButton
        icon={ChevronLeft}
        label="Previous year"
        size="sm"
        onclick={() => (pickerYear -= 1)}
      />
      <IconButton icon={ChevronRight} label="Next year" size="sm" onclick={() => (pickerYear += 1)} />
    {/if}
  </div>

  {#if view === 'days'}
    <div role="grid" aria-label="{MONTHS[monthIndex]} {year}" bind:this={grid}>
      <div role="row" class="grid grid-cols-7 gap-0.5 text-center">
        {#each weekdayLabels(weekStart) as label (label)}
          <div role="columnheader" class="pt-0.5 pb-1 text-2xs font-semibold text-faint">{label}</div>
        {/each}
      </div>
      {#each weeks as week (week[0]?.date)}
        <div role="row" class="grid grid-cols-7 gap-0.5 text-center">
          {#each week as d (d.date)}
            {@const off = isDisabled(d.date)}
            {@const isSelected = selected === d.date}
            {@const tinted = range !== undefined && inRange(range, d.date)}
            <div role="gridcell" aria-selected={isSelected || tinted}>
              <button
                type="button"
                data-date={d.date}
                tabindex={d.date === tabDate ? 0 : -1}
                aria-label="{long.format(new Date(`${d.date}T00:00:00Z`))}{marked?.has(d.date)
                  ? ', has entries'
                  : ''}"
                aria-current={d.date === today ? 'date' : undefined}
                aria-disabled={off || undefined}
                class="relative h-10.5 w-full rounded-sm text-xs tabular-nums transition-colors duration-(--dur) ease-out min-[720px]:h-8.5
                  {off || !d.inMonth ? 'text-faint' : 'text-ink'}
                  {off ? 'cursor-default opacity-35' : 'cursor-pointer hover:bg-hover'}
                  {!off && !d.inMonth ? 'opacity-50' : ''}
                  {d.date === today ? 'font-semibold shadow-[inset_0_0_0_1.5px_var(--cal-hue,var(--accent))]' : ''}
                  {isSelected ? 'bg-active' : ''} {tinted && !isSelected ? 'bg-accent-soft' : ''}"
                {onkeydown}
                onclick={() => pick(d.date)}
              >
                {d.day}
                {#if marked?.has(d.date)}
                  <span
                    aria-hidden="true"
                    class="absolute bottom-1 left-1/2 -ml-0.5 size-1 rounded-full bg-(--cal-hue,var(--accent))"
                  ></span>
                {/if}
              </button>
            </div>
          {/each}
        </div>
      {/each}
    </div>
  {:else}
    <div class="grid grid-cols-3 gap-1.5">
      {#each MONTHS as name, i (name)}
        <button
          type="button"
          class="h-10 cursor-pointer rounded-sm text-sm text-ink hover:bg-hover
            {pickerYear === year && i === monthIndex ? 'bg-active font-semibold' : ''}"
          aria-label="{name} {pickerYear}"
          onclick={() => chooseMonth(i)}
        >
          {name.slice(0, 3)}
        </button>
      {/each}
    </div>
  {/if}

  {#if showToday}
    <div class="mt-2 flex justify-between">
      <Button variant="ghost" size="sm" onclick={goToday}>Today</Button>
    </div>
  {/if}
</div>
