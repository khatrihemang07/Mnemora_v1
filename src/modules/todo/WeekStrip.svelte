<!--
  WeekStrip: Upcoming's sticky strip of one week. Seven days with a dot under each that has Tasks;
  today and days before it are disabled; ‹ › move by a week, to any future week but never before
  this one. `selected` is filled. Picking a day tells the page, which scrolls to it. The week starts
  on the Week start Setting. On a phone a sideways swipe on the strip changes week too.
-->
<script lang="ts">
  import ChevronLeft from '~icons/lucide/chevron-left';
  import ChevronRight from '~icons/lucide/chevron-right';
  import { viewSwipeStep } from '../../lib/swipe';
  import { addDays, type WeekStart } from '../../lib/ui/calendar';
  import IconButton from '../../lib/ui/IconButton.svelte';
  import { dayText, weekdayShort, weekStartOf } from '../../lib/ui/when-grid';

  interface Props {
    today: string;
    weekStart: WeekStart;
    /** Days that have Tasks. */
    busy: ReadonlySet<string>;
    selected: string | null;
    onpick: (date: string) => void;
  }
  let { today, weekStart, busy, selected, onpick }: Props = $props();

  /** Weeks after this one. */
  let offset = $state(0);

  /** Where a swipe began. */
  let from: { x: number; y: number; id: number } | null = null;
  function swipeEnd(e: PointerEvent) {
    const began = from;
    from = null;
    if (!began || began.id !== e.pointerId) return;
    // Left is the next week; the strip never goes before this one.
    const step = viewSwipeStep(e.clientX - began.x, e.clientY - began.y);
    if (step !== 0) offset = Math.max(0, offset + step);
  }

  const start = $derived(addDays(weekStartOf(today, weekStart), offset * 7));
  const days = $derived(Array.from({ length: 7 }, (_, i) => addDays(start, i)));
  const month = $derived(
    new Intl.DateTimeFormat('en-GB', { month: 'long', year: 'numeric', timeZone: 'UTC' }).format(
      new Date(`${start}T00:00:00Z`),
    ),
  );
</script>

<div
  class="grid grid-cols-[28px_repeat(7,1fr)_28px] items-center gap-0.5 border-b border-line py-1.5 phone:grid-cols-[24px_repeat(7,1fr)_24px]"
  role="group"
  aria-label="Week of {month}"
  title={month}
  style:touch-action="pan-y"
  onpointerdown={(e) => (from = e.pointerType === 'mouse' ? null : { x: e.clientX, y: e.clientY, id: e.pointerId })}
  onpointerup={swipeEnd}
  onpointercancel={() => (from = null)}
>
  <IconButton
    icon={ChevronLeft}
    label="Previous week"
    size="sm"
    disabled={offset === 0}
    onclick={() => (offset -= 1)}
    class="text-faint"
  />
  {#each days as date (date)}
    {@const off = date <= today}
    {@const chosen = selected === date}
    <button
      type="button"
      disabled={off}
      aria-pressed={chosen}
      aria-label={dayText(date, today) + (busy.has(date) ? ', has Tasks' : '')}
      onclick={() => onpick(date)}
      class="relative flex cursor-pointer flex-col items-center gap-px rounded-sm pt-1 pb-1.5 transition-colors duration-(--dur) ease-out
        hover:bg-hover disabled:cursor-default disabled:opacity-35 disabled:hover:bg-transparent"
    >
      <small class="text-2xs font-semibold text-faint uppercase">{weekdayShort(date)}</small>
      <b
        class="grid size-7 place-items-center rounded-full text-sm font-semibold tabular-nums
          {date === today ? 'shadow-[inset_0_0_0_1.5px_var(--hue-todo)]' : ''}
          {chosen ? 'bg-hue-todo text-on-accent' : ''}"
      >
        {Number(date.slice(8))}
      </b>
      <i
        aria-hidden="true"
        class="absolute bottom-0.5 size-1 rounded-full {busy.has(date) ? 'bg-hue-todo' : 'bg-transparent'}"
      ></i>
    </button>
  {/each}
  <IconButton
    icon={ChevronRight}
    label="Next week"
    size="sm"
    onclick={() => (offset += 1)}
    class="text-faint"
  />
</div>
