<!--
  WhenPicker: choose a Due date and time, compactly (about 420 x 280). A popover next to `anchor` on
  desktop, a bottom Sheet on a phone. From the top: a "Type a date" field (`parse` reads it; the
  default is small, a fuller reader plugs in), quick days on the left with their weekday, and on the
  right a rolling five-week calendar that starts at this week (‹ › move five weeks, a tiny month
  label sits on the 1st, past days are disabled, today is ringed, days with Tasks carry a dot that
  turns amber at three or more). A slim bar below holds the Time chip, whose panel floats over the
  calendar, anything the caller adds with `bar` (Repeat joins here), and a summary at the right.
  Picking a day sets it and closes; a time stays open. Controlled: `value` is what is set now,
  `onchange` gets the next value, and a time with no date is left for the caller's rules to date.
  `counts` holds open Tasks per day for the visible weeks, which `onrange` asks for. Dates are
  `YYYY-MM-DD`, times `HH:MM`. `showTime={false}` and `allowClear={false}` serve moving many
  Tasks to one date. `hue` is a Module hue token name.
-->
<script lang="ts">
  import { type Snippet, tick, untrack } from 'svelte';
  import CalendarArrowUp from '~icons/lucide/calendar-arrow-up';
  import CalendarCheck from '~icons/lucide/calendar-check';
  import ChevronLeft from '~icons/lucide/chevron-left';
  import ChevronRight from '~icons/lucide/chevron-right';
  import CircleSlash from '~icons/lucide/circle-slash';
  import Clock from '~icons/lucide/clock';
  import Sofa from '~icons/lucide/sofa';
  import Sun from '~icons/lucide/sun';
  import X from '~icons/lucide/x';
  import type { TimeFormat } from '../time/format';
  import { viewport } from '../viewport.svelte';
  import { moveFocus, type WeekStart } from './calendar';
  import IconButton from './IconButton.svelte';
  import Popover from './Popover.svelte';
  import Sheet from './Sheet.svelte';
  import { type ParsedWhen, simpleWhenParser, type WhenParser } from './when-parse';
  import { buildGrid, dayText, pageForDate, quickDays } from './when-grid';
  import { formatHm, timeSuggestions } from './when-time';

  export interface WhenValue {
    /** `YYYY-MM-DD`, or null for no date. */
    date: string | null;
    /** `HH:MM`, or null for no time. */
    time: string | null;
  }

  interface Props {
    open: boolean;
    /** The button the popover hangs off (desktop). */
    anchor: HTMLElement | null;
    value: WhenValue;
    /** Today, `YYYY-MM-DD`. */
    today: string;
    /** The time of day, `HH:MM`. */
    now: string;
    weekStart: WeekStart;
    timeFormat: TimeFormat;
    /** Open Tasks per day, for the dots. Days not in it have none. */
    counts?: ReadonlyMap<string, number>;
    onchange: (next: WhenValue) => void;
    /** The first and last day on show changed: load `counts` for them. */
    onrange?: (from: string, to: string) => void;
    title?: string;
    parse?: WhenParser;
    showTime?: boolean;
    allowClear?: boolean;
    hue?: string;
    restoreFocus?: () => void;
    /** More chips for the bar, after Time. */
    bar?: Snippet;
  }
  let {
    open = $bindable(),
    anchor,
    value,
    today,
    now,
    weekStart,
    timeFormat,
    counts,
    onchange,
    onrange,
    title = 'When',
    parse = simpleWhenParser,
    showTime = true,
    allowClear = true,
    hue = '--hue-todo',
    restoreFocus,
    bar,
  }: Props = $props();

  let page = $state(0);
  let typed = $state('');
  let panel = $state<'time' | null>(null);
  let timeText = $state('');
  let timeBad = $state(false);
  let root = $state<HTMLElement | null>(null);

  const grid = $derived(buildGrid(today, weekStart, page));
  const quick = $derived(quickDays(today, weekStart));
  const reading = $derived(typed.trim() ? parse(typed, { today, now, weekStart }) : null);
  const suggestions = $derived(timeSuggestions(timeText));

  // Each opening starts on the page that holds the set date, with nothing typed.
  $effect(() => {
    if (!open) return;
    untrack(() => {
      page = value.date ? pageForDate(value.date, today, weekStart) : 0;
      typed = '';
      timeText = '';
      timeBad = false;
      panel = null;
    });
  });
  $effect(() => {
    if (open) onrange?.(grid.start, grid.end);
  });
  const tabDate = $derived(
    value.date && value.date >= grid.start && value.date <= grid.end
      ? value.date
      : (grid.cells.find((c) => c.today)?.date ??
          grid.cells.find((c) => !c.past)?.date ??
          grid.start),
  );

  /** The label for a value: "Tomorrow · 9:00", "Fri 23 Oct", "9:00" or "No date". */
  function summarise(v: WhenValue): string {
    const time = v.time ? formatHm(v.time, timeFormat) : null;
    if (!v.date) return time ?? 'No date';
    const day =
      v.date === today ? 'Today' : v.date === quick[1]?.date ? 'Tomorrow' : dayText(v.date, today);
    return time ? `${day} · ${time}` : day;
  }

  function pickDay(date: string | null) {
    if (date !== null && date < today) return;
    // A date keeps the time already set; no date takes the time with it.
    onchange(date === null ? { date: null, time: null } : { date, time: value.time });
    open = false;
  }

  function applyTyped() {
    const read: ParsedWhen | null = reading;
    if (!read) return;
    onchange({ date: read.date ?? value.date, time: read.time ?? (read.date ? value.time : null) });
    open = false;
  }

  function onTypeKey(e: KeyboardEvent) {
    if (e.isComposing || e.key !== 'Enter') return;
    e.preventDefault();
    applyTyped();
  }

  async function openTime() {
    panel = 'time';
    timeText = '';
    timeBad = false;
    await tick();
    root?.querySelector<HTMLInputElement>('[data-when-time]')?.focus();
  }

  async function closeTime() {
    panel = null;
    await tick();
    root?.querySelector<HTMLElement>('[data-when-time-chip]')?.focus();
  }

  function setTime(time: string | null) {
    onchange({ date: value.date, time });
    void closeTime();
  }

  function onTimeKey(e: KeyboardEvent) {
    if (e.isComposing) return;
    if (e.key === 'Enter') {
      e.preventDefault();
      const first = suggestions[0];
      if (first) setTime(first);
      else timeBad = timeText.trim() !== '';
    }
  }

  async function onGridKey(e: KeyboardEvent) {
    if (e.altKey || e.ctrlKey || e.metaKey || !e.key.startsWith('Arrow')) return;
    const from = (e.target as HTMLElement).dataset.date;
    if (!from) return;
    const to = moveFocus(from, e.key, weekStart);
    if (!to || to < grid.start || to > grid.end) return;
    e.preventDefault();
    await tick();
    root?.querySelector<HTMLElement>(`[data-date="${to}"]`)?.focus();
  }

  /** Esc closes the Time panel first; only a second Esc leaves the picker. */
  function guardEscape(): boolean {
    if (panel === null) return false;
    void closeTime();
    return true;
  }

  const QUICK_ICONS = [CalendarCheck, Sun, Sofa, CalendarArrowUp];
  const QUICK_TONES = ['text-success', 'text-warning', 'text-accent', 'text-hue-ai'];
  const dotLabel = (n: number) => `${n} ${n === 1 ? 'Task' : 'Tasks'}`;
</script>

{#snippet body()}
  <div
    bind:this={root}
    class="flex w-full flex-col text-ink desktop:w-105"
    style="--wp-hue: var({hue})"
  >
    <div class="flex items-center gap-2 border-b border-line px-3 py-2 phone:py-2.5">
      <input
        data-when-type
        data-bare
        type="text"
        bind:value={typed}
        onkeydown={onTypeKey}
        aria-label="Type a date"
        placeholder="Type a date: fri, 28 oct, in 3 days"
        autocomplete="off"
        spellcheck="false"
        class="selectable min-w-0 flex-1 bg-transparent text-sm text-ink outline-none placeholder:text-faint phone:text-base"
      />
      {#if reading}
        <span class="shrink-0 text-xs font-semibold text-(--wp-hue)" aria-live="polite">
          {summarise({ date: reading.date, time: reading.time })} ↵
        </span>
      {:else if typed.trim()}
        <span class="shrink-0 text-xs text-faint" aria-live="polite">Not understood</span>
      {/if}
    </div>

    <div class="relative grid grid-cols-[104px_1fr] desktop:grid-cols-[148px_1fr]">
      <div class="flex flex-col border-r border-line p-1">
        {#each quick as q, i (q.key)}
          {@const Icon = QUICK_ICONS[i] ?? CalendarCheck}
          <button
            type="button"
            onclick={() => pickDay(q.date)}
            aria-pressed={value.date === q.date}
            class="grid cursor-pointer grid-cols-[18px_1fr_auto] items-center gap-1.75 rounded-sm px-1.75 py-1.25 text-left text-xs font-medium transition-colors duration-(--dur) ease-out hover:bg-hover phone:py-2
              {value.date === q.date ? 'bg-active' : ''}"
          >
            <Icon class="size-3.5 {QUICK_TONES[i]}" />
            <span class="whitespace-nowrap">{q.label}</span>
            <small class="text-2xs font-normal text-faint phone:hidden">{q.hint}</small>
          </button>
        {/each}
        {#if allowClear}
          <button
            type="button"
            onclick={() => pickDay(null)}
            aria-pressed={value.date === null && value.time === null}
            class="grid cursor-pointer grid-cols-[18px_1fr_auto] items-center gap-1.75 rounded-sm px-1.75 py-1.25 text-left text-xs font-medium transition-colors duration-(--dur) ease-out hover:bg-hover phone:py-2
              {value.date === null && value.time === null ? 'bg-active' : ''}"
          >
            <CircleSlash class="size-3.5 text-faint" />
            <span>No date</span>
          </button>
        {/if}
      </div>

      <div class="px-2 py-1.5">
        <div class="flex items-center pl-1">
          <b class="flex-1 text-xs font-semibold" aria-live="polite">{grid.heading}</b>
          <IconButton
            icon={ChevronLeft}
            label="Earlier weeks"
            size="sm"
            class="size-5.5 text-faint"
            disabled={!grid.canGoBack}
            onclick={() => (page -= 1)}
          />
          <IconButton
            icon={ChevronRight}
            label="Later weeks"
            size="sm"
            class="size-5.5 text-faint"
            onclick={() => (page += 1)}
          />
        </div>
        <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
        <div role="group" aria-label={grid.heading} class="grid grid-cols-7 gap-y-px" onkeydown={onGridKey}>
          {#each grid.weekdays as d, i (i)}
            <span aria-hidden="true" class="pt-px pb-0.5 text-center text-[9.5px] font-semibold text-faint">{d}</span>
          {/each}
          {#each grid.cells as cell (cell.date)}
            {@const n = counts?.get(cell.date) ?? 0}
            {@const chosen = value.date === cell.date}
            <button
              type="button"
              data-date={cell.date}
              tabindex={cell.date === tabDate ? 0 : -1}
              aria-pressed={chosen}
              aria-disabled={cell.past || undefined}
              aria-current={cell.today ? 'date' : undefined}
              aria-label="{dayText(cell.date, today)}{n ? `, ${dotLabel(n)}` : ''}"
              title="{dayText(cell.date, today)}{n ? ` · ${dotLabel(n)}` : ''}"
              onclick={() => pickDay(cell.date)}
              class="relative grid size-7 cursor-pointer place-items-center justify-self-center rounded-full text-xs tabular-nums transition-colors duration-(--dur) ease-out phone:size-7.5
                {cell.past ? 'cursor-default text-faint opacity-35' : 'hover:bg-hover'}
                {cell.today && !chosen ? 'font-bold text-(--wp-hue) shadow-[inset_0_0_0_1.2px_var(--wp-hue)]' : ''}
                {chosen ? 'bg-(--wp-hue) font-semibold text-on-accent hover:bg-(--wp-hue)' : ''}"
            >
              {#if cell.monthLabel}
                <em class="absolute -top-0.75 text-[8px] font-bold tracking-wide text-(--wp-hue) uppercase not-italic">
                  {cell.monthLabel}
                </em>
              {/if}
              {cell.day}
              {#if n > 0}
                <span
                  aria-hidden="true"
                  class="absolute bottom-0.5 left-1/2 -ml-[1.5px] size-[3px] rounded-full {chosen
                    ? 'bg-on-accent'
                    : n >= 3
                      ? 'bg-warning'
                      : 'bg-faint'}"
                ></span>
              {/if}
            </button>
          {/each}
        </div>
      </div>

      {#if panel === 'time'}
        <div class="absolute inset-x-1.5 inset-y-1 z-10 flex flex-col overflow-hidden rounded-md border border-line bg-pop shadow-pop">
          <div class="flex items-center gap-1 border-b border-line py-1 pr-1 pl-3">
            <b class="flex flex-1 items-center gap-1.5 text-xs font-semibold"><Clock class="size-3.5" />Time</b>
            {#if value.time}
              <button
                type="button"
                onclick={() => setTime(null)}
                class="cursor-pointer rounded-sm px-1.5 py-0.5 text-xs font-semibold text-muted hover:bg-hover hover:text-danger"
              >
                Remove
              </button>
            {/if}
            <IconButton icon={X} label="Close" size="sm" onclick={() => void closeTime()} />
          </div>
          <input
            data-when-time
            data-bare
            type="text"
            bind:value={timeText}
            oninput={() => (timeBad = false)}
            onkeydown={onTimeKey}
            aria-label="Type a time"
            placeholder="Type a time: 9, 930p, 21:30"
            autocomplete="off"
            spellcheck="false"
            class="selectable border-b border-line bg-transparent px-3 py-2 text-sm text-ink outline-none placeholder:text-faint phone:text-base"
          />
          <div class="flex flex-col overflow-y-auto p-1" role="listbox" aria-label="Matching times">
            {#each suggestions as s, i (s)}
              <button
                type="button"
                role="option"
                aria-selected={i === 0}
                onclick={() => setTime(s)}
                class="flex cursor-pointer items-center gap-2 rounded-sm px-2.5 py-1.5 text-left text-sm hover:bg-hover {i === 0 ? 'bg-active' : ''}"
              >
                <Clock class="size-3.5 text-faint" />{formatHm(s, timeFormat)}
              </button>
            {/each}
            {#if timeBad || (timeText.trim() && suggestions.length === 0)}
              <p class="px-2.5 py-1.5 text-xs text-faint">Not a time we can read</p>
            {/if}
          </div>
        </div>
      {/if}
    </div>

    <div class="flex items-center gap-1 border-t border-line px-1.5 py-1.25 phone:py-2">
      {#if showTime}
        <button
          type="button"
          data-when-time-chip
          onclick={() => (panel === 'time' ? void closeTime() : void openTime())}
          aria-expanded={panel === 'time'}
          class="inline-flex h-6.5 max-w-40 cursor-pointer items-center gap-1.25 rounded-full border px-2.25 text-xs font-medium whitespace-nowrap transition-colors duration-(--dur) ease-out
            {value.time
            ? 'border-transparent bg-[color-mix(in_oklch,var(--wp-hue)_14%,transparent)] text-(--wp-hue)'
            : 'border-line text-muted hover:border-faint'}
            {panel === 'time' ? 'shadow-[0_0_0_2px_var(--ring)]' : ''}"
        >
          <Clock class="size-3.5" />
          {value.time ? formatHm(value.time, timeFormat) : 'Time'}
        </button>
      {/if}
      {@render bar?.()}
      <span class="mr-1 ml-auto truncate text-xs whitespace-nowrap text-faint" aria-live="polite">
        {summarise(value)}
      </span>
    </div>
  </div>
{/snippet}

{#if viewport.isPhone}
  <Sheet {open} {title} onclose={() => (open = false)} {restoreFocus} closable {guardEscape}>
    {@render body()}
  </Sheet>
{:else}
  <Popover
    bind:open
    {anchor}
    label={title}
    focus="[data-when-type]"
    bare
    {restoreFocus}
    {guardEscape}
  >
    {@render body()}
  </Popover>
{/if}
