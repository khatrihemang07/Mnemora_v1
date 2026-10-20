<!--
  DatePicker: choose a date. A popover next to `anchor` on desktop, a bottom Sheet on a phone. Today
  and Yesterday chips sit above a month Calendar. `mode="single"` calls `onpick(date)` and closes;
  `mode="range"` takes two picks (see `pickRange`), shows what is chosen and calls `onrange` once the
  range is complete. `open` is bindable. Dates are `YYYY-MM-DD`. For browsing rather than choosing
  (the Journal's calendar), `chips={false}` hides the quick chips, `closeOnPick={false}` keeps it
  open after a pick, `showToday` adds the Calendar's Today button and `footer` adds content below.
-->
<script lang="ts">
  import { type Snippet, untrack } from 'svelte';
  import { viewport } from '../viewport.svelte';
  import Calendar from './Calendar.svelte';
  import { addDays, type DateRange, EMPTY_RANGE, pickRange, type WeekStart } from './calendar';
  import Popover from './Popover.svelte';
  import Sheet from './Sheet.svelte';

  interface Props {
    open: boolean;
    /** The button the popover hangs off (desktop). */
    anchor: HTMLElement | null;
    title: string;
    mode?: 'single' | 'range';
    today: string;
    weekStart: WeekStart;
    /** Single mode: the date shown selected and the month opened on. */
    value?: string | null;
    /** Range mode: the range shown when opening. */
    range?: DateRange;
    marked?: ReadonlySet<string>;
    hue?: string;
    onpick?: (date: string) => void;
    onrange?: (range: DateRange) => void;
    /** Called when the visible month changes, e.g. to load `marked` for it. */
    onmonthchange?: (month: string) => void;
    /** Where focus goes when the popover closes; the anchor by default. */
    restoreFocus?: () => void;
    /** Single mode: show the Today / Yesterday chips. */
    chips?: boolean;
    /** Single mode: close after a pick. */
    closeOnPick?: boolean;
    showToday?: boolean;
    ontoday?: () => void;
    /** Content under the Calendar, e.g. a note about the day just picked. */
    footer?: Snippet;
    /** The Calendar's dots follow the fetch for `marked`; `disabled` greys days out. */
    disabled?: (date: string) => boolean;
  }
  let {
    open = $bindable(),
    anchor,
    title,
    mode = 'single',
    today,
    weekStart,
    value = null,
    range = EMPTY_RANGE,
    marked,
    hue,
    onpick,
    onrange,
    onmonthchange,
    restoreFocus,
    chips: showChips = true,
    closeOnPick = true,
    showToday = false,
    ontoday,
    footer,
    disabled,
  }: Props = $props();

  let month = $state(untrack(() => today.slice(0, 7)));
  let draft = $state<DateRange>(EMPTY_RANGE);

  // Each opening starts from the current value (or the range's start) in its own month.
  $effect(() => {
    if (!open) return;
    untrack(() => {
      draft = mode === 'range' ? range : EMPTY_RANGE;
      month = (mode === 'range' ? range.from : value)?.slice(0, 7) ?? today.slice(0, 7);
    });
  });
  $effect(() => {
    onmonthchange?.(month);
  });

  function pick(date: string) {
    if (mode === 'single') {
      if (closeOnPick) open = false;
      onpick?.(date);
      return;
    }
    draft = pickRange(draft, date);
    if (draft.to !== null) {
      open = false;
      onrange?.(draft);
    }
  }

  const chips = $derived([
    { label: 'Today', date: today },
    { label: 'Yesterday', date: addDays(today, -1) },
  ]);
</script>

{#snippet body()}
  <div class="flex flex-col gap-2.5">
    {#if mode === 'single'}
      {#if showChips}
      <div class="flex gap-1.5">
        {#each chips as chip (chip.label)}
          <button
            type="button"
            class="inline-flex cursor-pointer items-center rounded-full border px-2.5 py-1 text-xs font-semibold text-ink transition-colors duration-(--dur) ease-out hover:border-faint
              {value === chip.date ? 'border-transparent bg-active' : 'border-line bg-surface-2'}"
            onclick={() => pick(chip.date)}
          >
            {chip.label}
          </button>
        {/each}
      </div>
      {/if}
    {:else}
      <p class="text-xs text-muted" aria-live="polite">
        {#if draft.from === null || draft.to !== null}Pick the first day{:else}Now pick the last day{/if}
      </p>
    {/if}
    <Calendar
      bind:month
      {today}
      selected={mode === 'single' ? value : null}
      range={mode === 'range' ? draft : undefined}
      {marked}
      {weekStart}
      {hue}
      {disabled}
      {showToday}
      {ontoday}
      onpick={pick}
      class="w-full desktop:w-67"
    />
    {@render footer?.()}
  </div>
{/snippet}

{#if viewport.isPhone}
  <Sheet {open} {title} onclose={() => (open = false)} {restoreFocus} closable>{@render body()}</Sheet>
{:else}
  <Popover bind:open {anchor} label={title} focus="[data-date][tabindex='0']" {restoreFocus}>{@render body()}</Popover>
{/if}
