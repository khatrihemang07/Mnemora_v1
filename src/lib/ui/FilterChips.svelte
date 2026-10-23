<!--
  FilterChips: a row of small pills that filter a list, one chosen at a time (Activity: All · This
  device · each paired device). A radio group: the chosen pill is tinted with the Module `hue`;
  arrow keys move between pills. Wraps on a narrow screen. To open a picker use `Chip`; to pick a
  value in a form use `SegmentedControl`.
-->
<script lang="ts">
  interface Option {
    value: string;
    label: string;
  }
  interface Props {
    options: Option[];
    value: string;
    label: string;
    onchange: (value: string) => void;
    /** A Module hue token name (`--hue-sync`) for the chosen pill; the brand accent by default. */
    hue?: string;
    class?: string;
  }
  let { options, value, label, onchange, hue = '--accent', class: cls = '' }: Props = $props();

  function onkeydown(e: KeyboardEvent, index: number) {
    const step = e.key === 'ArrowRight' || e.key === 'ArrowDown' ? 1 : e.key === 'ArrowLeft' || e.key === 'ArrowUp' ? -1 : 0;
    if (step === 0) return;
    e.preventDefault();
    const next = options[(index + step + options.length) % options.length];
    if (!next) return;
    onchange(next.value);
    (e.currentTarget as HTMLElement).parentElement
      ?.querySelectorAll<HTMLElement>('[role="radio"]')
      [(index + step + options.length) % options.length]?.focus();
  }
</script>

<div
  role="radiogroup"
  aria-label={label}
  style="--chip-hue: var({hue})"
  class="flex flex-wrap gap-1.5 {cls}"
>
  {#each options as option, i (option.value)}
    {@const on = option.value === value}
    <button
      type="button"
      role="radio"
      aria-checked={on}
      tabindex={on ? 0 : -1}
      class="cursor-pointer rounded-full border px-2.5 py-1 text-xs font-semibold whitespace-nowrap select-none
        transition-[transform,background-color,border-color] duration-(--dur) ease-out active:scale-[0.96]
        {on
        ? 'border-transparent bg-[color-mix(in_oklch,var(--chip-hue)_14%,transparent)] text-(--chip-hue)'
        : 'border-line-strong bg-surface-2 text-ink hover:border-faint'}"
      onclick={() => onchange(option.value)}
      onkeydown={(e) => onkeydown(e, i)}
    >
      {option.label}
    </button>
  {/each}
</div>
