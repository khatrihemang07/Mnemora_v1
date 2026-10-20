<!--
  JournalCalendar: the Journal's go-to-date calendar (header button, ⌘G). A popover on desktop and a
  bottom Sheet on a phone (DatePicker). It opens on the month of the Page in view with that day
  highlighted; coral dots mark days that have Entries (the month shown plus its neighbours, whose
  grey days show in the grid). Tapping a day with Entries closes it and asks the feed to scroll
  there; tapping an empty day says so and offers "Write on this day".
-->
<script lang="ts">
  import { untrack } from 'svelte';
  import PenLine from '~icons/lucide/pen-line';
  import Button from '../../lib/ui/Button.svelte';
  import type { WeekStart } from '../../lib/ui/calendar';
  import DatePicker from '../../lib/ui/DatePicker.svelte';
  import { dayLabel } from './dates';
  import { useMarkedDays } from './marked-days.svelte';

  interface Props {
    open: boolean;
    anchor: HTMLElement | null;
    today: string;
    weekStart: WeekStart;
    /** The day highlighted on opening: the Page in view. */
    highlight: string;
    /** Scroll the feed to this day's Page (it has Entries, or is Today). */
    onshow: (date: string) => void;
    onwrite: (date: string) => void;
  }
  let { open = $bindable(), anchor, today, weekStart, highlight, onshow, onwrite }: Props = $props();

  let month = $state(untrack(() => today.slice(0, 7)));
  /** The day last tapped; null until one is. Moves the highlight and decides the empty-day note. */
  let tapped = $state<string | null>(null);
  // "Write on this day": closing hands focus to the composer, not back to the calendar button.
  let writing: string | null = null;

  const days = useMarkedDays(() => ({ month, open }));
  const marked = $derived(days.marked);

  // Forget the tap on closing, so the next opening starts from the Page in view.
  $effect(() => {
    if (!open) tapped = null;
  });

  function pick(date: string) {
    tapped = date;
    if (marked.has(date) || date === today) {
      open = false;
      onshow(date);
    }
  }

  const empty = $derived(tapped !== null && tapped !== today && !marked.has(tapped) ? tapped : null);
</script>

<DatePicker
  bind:open
  {anchor}
  restoreFocus={() => {
    if (writing) onwrite(writing);
    else anchor?.focus();
    writing = null;
  }}
  title="Go to date"
  {today}
  {weekStart}
  value={tapped ?? highlight}
  {marked}
  hue="--hue-journal"
  chips={false}
  closeOnPick={false}
  showToday
  ontoday={() => {
    open = false;
    onshow(today);
  }}
  onpick={pick}
  onmonthchange={(m) => (month = m)}
>
  {#snippet footer()}
    {#if empty}
      <div class="flex flex-col gap-2 rounded-sm bg-surface-2 p-2.5 text-xs text-ink" aria-live="polite">
        <span>No entries on <b>{dayLabel(empty, today)}</b></span>
        <Button
          variant="primary"
          size="sm"
          onclick={() => {
            writing = empty;
            open = false;
          }}
        >
          <PenLine class="size-3.5" /> Write on this day
        </Button>
      </div>
    {/if}
  {/snippet}
</DatePicker>
