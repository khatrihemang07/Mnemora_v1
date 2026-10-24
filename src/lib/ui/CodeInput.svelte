<!--
  CodeInput: a short numeric code in one box per digit (8, in two groups of 4). Typing advances to the
  next box, Backspace goes back, a paste fills every box, and the phone gets the numeric keyboard.
  Bind `value` (digits only); `length` is the number of boxes. `oncomplete` runs when the last digit is typed or pasted. Bind `ref` to
  focus the active box from outside. The editing rules are in code-input.ts.
-->
<script lang="ts">
  import { tick } from 'svelte';
  import { backspaceAt, boxes, CODE_LENGTH, newlyTyped, pasteInto, typeInto } from './code-input';

  interface Props {
    value: string;
    /** Digits in the code; the boxes split in two groups. */
    length?: number;
    /** The accessible name of the whole code. */
    label: string;
    invalid?: boolean;
    disabled?: boolean;
    autofocus?: boolean;
    oncomplete?: (value: string) => void;
    class?: string;
  }
  let {
    value = $bindable(),
    length = CODE_LENGTH,
    label,
    invalid = false,
    disabled = false,
    autofocus = false,
    oncomplete,
    class: cls = '',
  }: Props = $props();

  const GROUP = $derived(length / 2);
  let inputs = $state<(HTMLInputElement | null)[]>([]);
  const digits = $derived(boxes(value, length));

  async function apply(edit: { value: string; focus: number }) {
    const was = value;
    value = edit.value;
    await tick();
    inputs[edit.focus]?.focus();
    inputs[edit.focus]?.select();
    if (value.length === length && was !== value) oncomplete?.(value);
  }

  function oninput(e: Event, i: number) {
    const box = e.currentTarget as HTMLInputElement;
    const typed = newlyTyped(box.value, digits[i] ?? '');
    // The box always shows exactly what the state says, whatever the keyboard put in it.
    box.value = digits[i] ?? '';
    void apply(typeInto(value, i, typed, length));
  }

  function onkeydown(e: KeyboardEvent, i: number) {
    if (e.key === 'Backspace') {
      e.preventDefault();
      void apply(backspaceAt(value, i));
    } else if (e.key === 'ArrowLeft' && i > 0) {
      e.preventDefault();
      inputs[i - 1]?.focus();
    } else if (e.key === 'ArrowRight' && i < Math.min(value.length, length - 1)) {
      e.preventDefault();
      inputs[i + 1]?.focus();
    }
  }

  function onpaste(e: ClipboardEvent) {
    e.preventDefault();
    const edit = pasteInto(e.clipboardData?.getData('text') ?? '', length);
    if (edit) void apply(edit);
  }

  // A box after the first empty one would leave a gap, so focus goes to the first empty box instead.
  function onfocus(e: FocusEvent, i: number) {
    const box = e.currentTarget as HTMLInputElement;
    if (i > value.length) inputs[Math.min(value.length, length - 1)]?.focus();
    else box.select();
  }

  $effect(() => {
    if (autofocus) inputs[Math.min(value.length, length - 1)]?.focus();
  });
</script>

<div role="group" aria-label={label} class="flex justify-center gap-1.5 {cls}">
  {#each digits as digit, i (i)}
    {#if i === GROUP}<span aria-hidden="true" class="w-2"></span>{/if}
    <input
      bind:this={inputs[i]}
      type="text"
      inputmode="numeric"
      pattern="[0-9]*"
      autocomplete={i === 0 ? 'one-time-code' : 'off'}
      aria-label="Digit {i + 1} of {length}"
      aria-invalid={invalid}
      {disabled}
      value={digit}
      oninput={(e) => oninput(e, i)}
      onkeydown={(e) => onkeydown(e, i)}
      {onpaste}
      onfocus={(e) => onfocus(e, i)}
      class="selectable h-11 w-8 min-w-0 rounded-md border-[1.5px] bg-surface text-center font-mono text-xl font-semibold text-ink transition-colors duration-(--dur) ease-out disabled:opacity-50 desktop:w-[34px]
        {invalid ? 'border-danger' : 'border-line focus:border-hue-sync'}
        focus:shadow-[0_0_0_3px_var(--hue-sync-soft)]"
    />
  {/each}
</div>
