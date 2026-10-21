<!--
  TaskCheckbox: the Task's checkbox, a rounded square turned 45° (a diamond) with an upright tick.
  It is the Task's alone: Checklist items inside notes keep their plain square. `tone` colours the
  edge and fill (a Priority ring, or the Overdue tint); the Todo hue marks the done state and the
  hover. A Task marked Won't do (`wont`, with `checked`) is a grey diamond holding a ⊗, never the
  done colour. The button is a larger hit area than the diamond, and the diamond carries the focus
  ring. `label` is the accessible name; `onchange` receives the next state.
-->
<script lang="ts">
  import Check from '~icons/lucide/check';
  import X from '~icons/lucide/x';

  type Tone = 'plain' | 'p1' | 'p2' | 'p3' | 'overdue';
  interface Props {
    checked: boolean;
    label: string;
    onchange?: (checked: boolean) => void;
    tone?: Tone;
    /** Closed as Won't do rather than done. */
    wont?: boolean;
    disabled?: boolean;
    class?: string;
  }
  let {
    checked,
    label,
    onchange,
    tone = 'plain',
    wont = false,
    disabled = false,
    class: cls = '',
  }: Props = $props();

  // Edge and fill per tone. Done always wears the Todo hue.
  const LOOK: Record<Tone, string> = {
    plain: '--cb-edge: var(--faint); --cb-fill: transparent',
    p1: '--cb-edge: var(--danger); --cb-fill: var(--danger-soft)',
    p2: '--cb-edge: var(--warning); --cb-fill: color-mix(in oklch, var(--warning) 14%, transparent)',
    p3: '--cb-edge: var(--accent); --cb-fill: var(--accent-soft)',
    overdue: '--cb-edge: color-mix(in oklch, var(--danger) 70%, var(--faint)); --cb-fill: transparent',
  };
</script>

<button
  type="button"
  role="checkbox"
  aria-checked={checked}
  aria-label={label}
  {disabled}
  onclick={() => onchange?.(!checked)}
  class="group relative grid size-6.5 shrink-0 cursor-pointer place-items-center focus-visible:shadow-none focus-visible:outline-none
    before:absolute before:-inset-1.5 before:content-[''] phone:size-7.5 {cls}"
>
  <span
    style={LOOK[tone]}
    data-checked={checked && !wont ? '' : undefined}
    data-wont={checked && wont ? '' : undefined}
    class="grid size-4.5 scale-[0.86] rotate-45 place-items-center rounded-[4px] border-[1.6px] border-(--cb-edge) bg-(--cb-fill) text-transparent
      transition-[background-color,border-color,box-shadow,color] duration-(--dur) ease-out phone:size-5.5
      group-hover:border-hue-todo group-hover:text-hue-todo group-hover:shadow-[0_0_0_4px_color-mix(in_oklch,var(--hue-todo)_14%,transparent)]
      group-focus-visible:shadow-[0_0_0_4px_var(--ring)] group-focus-visible:outline-[1.5px] group-focus-visible:outline-offset-1 group-focus-visible:outline-accent
      group-active:scale-[0.8] data-checked:border-hue-todo data-checked:bg-hue-todo data-checked:text-on-accent
      data-wont:border-faint data-wont:bg-surface-2 data-wont:text-muted"
  >
    {#if wont}<X class="size-3 -rotate-45" />{:else}<Check class="size-3 -rotate-45" />{/if}
  </span>
</button>
