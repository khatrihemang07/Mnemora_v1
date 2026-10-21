<!--
  WhenRepeatPanel: the Repeat panel of the WhenPicker. It floats over the calendar like the Time
  panel. Presets sit in two columns (Every day, Every 2/3/4 days, Every weekday, Weekly on ‹weekday›,
  Monthly on the ‹date›, Yearly on ‹date›); "Custom…" opens every N (1 to 99) days, weeks (with
  weekday toggles), months or years. Below, "From the due date" or "From the day it's done" (the
  latter only with an interval, not chosen weekdays) and a "Next:" line with the next three dates.
  Controlled: `rule` is what is set, `onchange` gets the next rule (or null to remove it). `date` is
  the Due date the presets are worded for. Rules and dates come from `when-repeat.ts`.
-->
<script lang="ts">
  import { untrack } from 'svelte';
  import ChevronLeft from '~icons/lucide/chevron-left';
  import Repeat from '~icons/lucide/repeat';
  import X from '~icons/lucide/x';
  import IconButton from './IconButton.svelte';
  import { dayText } from './when-grid';
  import {
    afterAllowed,
    dayBit,
    MAX_EVERY,
    presetKey,
    preview,
    type RepeatFreq,
    type RepeatPreset,
    type RepeatRule,
    repeatPresets,
    WEEK_ORDER,
  } from './when-repeat';

  interface Props {
    rule: RepeatRule | null;
    /** The Due date (or the day the Repeat would start on). */
    date: string;
    today: string;
    onchange: (next: RepeatRule | null) => void;
    onclose: () => void;
  }
  let { rule, date, today, onchange, onclose }: Props = $props();

  const FREQS: { value: RepeatFreq; one: string; many: string }[] = [
    { value: 'day', one: 'Day', many: 'Days' },
    { value: 'week', one: 'Week', many: 'Weeks' },
    { value: 'month', one: 'Month', many: 'Months' },
    { value: 'year', one: 'Year', many: 'Years' },
  ];
  const LETTERS: Record<number, string> = { 0: 'S', 1: 'M', 2: 'T', 3: 'W', 4: 'T', 5: 'F', 6: 'S' };
  const NAMES: Record<number, string> = {
    0: 'Sunday',
    1: 'Monday',
    2: 'Tuesday',
    3: 'Wednesday',
    4: 'Thursday',
    5: 'Friday',
    6: 'Saturday',
  };

  const DEFAULT: RepeatRule = { freq: 'week', every: 1, days: null, mode: 'fixed' };

  // What the panel shows while it is open: the rule being edited, kept in step with the set one.
  let draft = $state<RepeatRule>(untrack(() => rule ?? DEFAULT));
  let view = $state<'presets' | 'custom'>(
    untrack(() => (rule && presetKey(rule) === null ? 'custom' : 'presets')),
  );
  let everyText = $state(untrack(() => String((rule ?? DEFAULT).every)));
  $effect(() => {
    if (!rule) return;
    untrack(() => {
      draft = rule;
      if (!document.activeElement?.hasAttribute('data-when-every')) everyText = String(rule.every);
    });
  });

  const presets = $derived(repeatPresets(date));
  const selected = $derived(presetKey(rule));
  const days = $derived(draft.days ?? 0);
  const dates = $derived(rule ? preview(rule, date, 3) : []);

  function apply(next: RepeatRule) {
    draft = next;
    onchange(next);
  }

  function pickPreset(p: RepeatPreset) {
    // Keep counting from the day it's done when that still makes sense; weekdays need a due date.
    const after = rule?.mode === 'after' && afterAllowed(p.rule);
    apply({ ...p.rule, mode: after ? 'after' : 'fixed' });
  }

  function setMode(mode: RepeatRule['mode']) {
    if (!rule || (mode === 'after' && !afterAllowed(rule))) return;
    apply({ ...rule, mode });
  }

  function onEvery(e: Event) {
    const text = (e.currentTarget as HTMLInputElement).value.replace(/\D/g, '').slice(0, 2);
    everyText = text;
    const n = Number(text);
    if (n >= 1 && n <= MAX_EVERY) apply({ ...draft, every: n });
  }

  function stepEvery(e: KeyboardEvent) {
    if (e.key !== 'ArrowUp' && e.key !== 'ArrowDown') return;
    e.preventDefault();
    const n = Math.min(MAX_EVERY, Math.max(1, draft.every + (e.key === 'ArrowUp' ? 1 : -1)));
    everyText = String(n);
    apply({ ...draft, every: n });
  }

  function setFreq(freq: RepeatFreq) {
    if (freq === draft.freq) return;
    apply({ freq, every: draft.every, days: null, mode: draft.mode });
  }

  function toggleDay(weekday: number) {
    const mask = days ^ dayBit(weekday);
    // Chosen weekdays cannot count from the day it's done.
    apply({ ...draft, days: mask || null, mode: mask ? 'fixed' : draft.mode });
  }

  const unit = (f: (typeof FREQS)[number]) => (draft.every === 1 ? f.one : f.many);

  const btn =
    'cursor-pointer rounded-sm px-2 text-left text-xs font-medium transition-colors duration-(--dur) ease-out hover:bg-hover';
</script>

<div
  data-when-repeat
  class="absolute inset-x-1.5 inset-y-1 z-10 flex flex-col overflow-hidden rounded-md border border-line bg-pop shadow-pop"
>
  <div class="flex items-center gap-1 border-b border-line py-1 pr-1 pl-2">
    {#if view === 'custom'}
      <IconButton icon={ChevronLeft} label="Back to presets" size="sm" onclick={() => (view = 'presets')} />
    {/if}
    <b class="flex flex-1 items-center gap-1.5 pl-1 text-xs font-semibold">
      <Repeat class="size-3.5" />{view === 'custom' ? 'Custom repeat' : 'Repeat'}
    </b>
    {#if view === 'presets'}
      <button
        type="button"
        onclick={() => (view = 'custom')}
        aria-pressed={rule !== null && selected === null}
        class="cursor-pointer rounded-sm px-1.5 py-0.5 text-xs font-semibold text-(--wp-hue) hover:bg-hover
          {rule !== null && selected === null ? 'bg-active' : ''}"
      >
        Custom…
      </button>
    {/if}
    {#if rule}
      <button
        type="button"
        onclick={() => onchange(null)}
        class="cursor-pointer rounded-sm px-1.5 py-0.5 text-xs font-semibold text-muted hover:bg-hover hover:text-danger"
      >
        Remove
      </button>
    {/if}
    <IconButton icon={X} label="Close" size="sm" onclick={onclose} />
  </div>

  <div class="flex min-h-0 flex-1 flex-col gap-1.5 overflow-y-auto p-1.5">
    {#if view === 'presets'}
      <div class="grid grid-cols-2 gap-px" role="group" aria-label="Repeat presets">
        {#each presets as p (p.key)}
          <button
            type="button"
            aria-pressed={selected === p.key}
            onclick={() => pickPreset(p)}
            class="{btn} flex h-6.5 items-center justify-between gap-1 phone:h-8 {selected === p.key ? 'bg-active' : ''}"
          >
            <span class="truncate">{p.label}</span>
            {#if selected === p.key}<span aria-hidden="true" class="text-(--wp-hue)">✓</span>{/if}
          </button>
        {/each}
      </div>
    {:else}
      <div class="flex flex-col gap-1.5 px-1 pt-0.5">
        <label class="flex items-center gap-2 text-xs">
          <span class="text-muted">Every</span>
          <input
            data-when-every
            data-bare
            type="text"
            inputmode="numeric"
            autocomplete="off"
            value={everyText}
            oninput={onEvery}
            onkeydown={stepEvery}
            onblur={() => (everyText = String(draft.every))}
            aria-label="Every how many (1 to 99)"
            class="selectable h-6.5 w-11 rounded-sm border border-line bg-transparent text-center text-sm text-ink tabular-nums outline-none focus:border-faint phone:h-8 phone:text-base"
          />
        </label>
        <div class="grid grid-cols-4 gap-0.5 rounded-sm bg-surface-2 p-0.5" role="group" aria-label="Unit">
          {#each FREQS as f (f.value)}
            <button
              type="button"
              aria-pressed={draft.freq === f.value}
              onclick={() => setFreq(f.value)}
              class="cursor-pointer rounded-sm py-1 text-xs font-medium text-muted transition-colors duration-(--dur) ease-out phone:py-1.5
                {draft.freq === f.value ? 'bg-surface text-ink shadow-card' : 'hover:text-ink'}"
            >
              {unit(f)}
            </button>
          {/each}
        </div>
        {#if draft.freq === 'week'}
          <div class="flex items-center justify-between" role="group" aria-label="On these days">
            {#each WEEK_ORDER as d (d)}
              <button
                type="button"
                aria-pressed={(days & dayBit(d)) !== 0}
                aria-label={NAMES[d]}
                title={NAMES[d]}
                onclick={() => toggleDay(d)}
                class="grid size-6.5 cursor-pointer place-items-center rounded-full text-xs font-semibold transition-colors duration-(--dur) ease-out phone:size-8
                  {(days & dayBit(d)) !== 0
                  ? 'bg-(--wp-hue) text-on-accent'
                  : 'text-muted hover:bg-hover'}"
              >
                {LETTERS[d]}
              </button>
            {/each}
          </div>
          {#if days === 0}
            <p class="text-2xs text-faint">No day chosen: the same weekday as the due date.</p>
          {/if}
        {/if}
      </div>
    {/if}

    {#if rule}
      <div class="mt-auto flex flex-col gap-1.5 px-1">
        <div class="grid grid-cols-2 gap-0.5 rounded-sm bg-surface-2 p-0.5" role="group" aria-label="Count from">
          <button
            type="button"
            aria-pressed={rule.mode === 'fixed'}
            onclick={() => setMode('fixed')}
            class="cursor-pointer rounded-sm py-1 text-xs font-medium transition-colors duration-(--dur) ease-out phone:py-1.5
              {rule.mode === 'fixed' ? 'bg-surface text-ink shadow-card' : 'text-muted hover:text-ink'}"
          >
            From the due date
          </button>
          <button
            type="button"
            aria-pressed={rule.mode === 'after'}
            disabled={!afterAllowed(rule)}
            title={afterAllowed(rule) ? undefined : 'Needs an interval, not chosen weekdays'}
            onclick={() => setMode('after')}
            class="rounded-sm py-1 text-xs font-medium transition-colors duration-(--dur) ease-out phone:py-1.5
              {rule.mode === 'after' ? 'bg-surface text-ink shadow-card' : 'text-muted'}
              {afterAllowed(rule) ? 'cursor-pointer hover:text-ink' : 'cursor-not-allowed opacity-40'}"
          >
            From the day it's done
          </button>
        </div>
        {#if dates.length > 0}
          <p class="truncate text-xs text-faint" aria-live="polite">
            <span class="font-semibold text-muted">Next:</span>
            {dates.map((d) => dayText(d, today)).join(' · ')}
          </p>
        {/if}
      </div>
    {/if}
  </div>
</div>
