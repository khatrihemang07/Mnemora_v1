<!--
  AutoTextarea: a frameless text field that grows with its text instead of scrolling: a Task's title
  in the sheet, which may wrap but is one line of text. Bind `value`; `label` is the accessible
  name. Enter never inserts a line break (the caller handles it through `onkeydown`); with
  `multiline`, Shift+Enter does (a comment); with `enterBreaks`, Enter does too. Bind `ref` to focus it.
-->
<script lang="ts">
  import type { HTMLTextareaAttributes } from 'svelte/elements';
  import { enterBreaksLine } from './auto-textarea-enter';

  interface Props extends Omit<HTMLTextareaAttributes, 'value'> {
    value: string;
    label: string;
    ref?: HTMLTextAreaElement | null;
    /** Shift+Enter adds a line break. */
    multiline?: boolean;
    /** Enter adds a line break too (no key sends, so the caller's button does). */
    enterBreaks?: boolean;
  }
  let {
    value = $bindable(),
    label,
    ref = $bindable(null),
    multiline = false,
    enterBreaks = false,
    class: cls = '',
    onkeydown,
    ...rest
  }: Props = $props();

  function fit(el: HTMLTextAreaElement) {
    el.style.height = 'auto';
    el.style.height = `${el.scrollHeight}px`;
  }

  // Fit again when the text changes or the width does (the window or the sheet resized).
  $effect(() => {
    void value;
    if (ref) fit(ref);
  });
  $effect(() => {
    const el = ref;
    if (!el) return;
    const watch = new ResizeObserver(() => fit(el));
    watch.observe(el);
    return () => watch.disconnect();
  });
</script>

<textarea
  bind:this={ref}
  bind:value
  rows="1"
  aria-label={label}
  class="selectable block w-full resize-none overflow-hidden bg-transparent text-ink outline-none placeholder:text-muted {cls}"
  onkeydown={(e) => {
    if (e.key === 'Enter' && !enterBreaksLine({ shiftKey: e.shiftKey, multiline, enterBreaks })) {
      e.preventDefault();
    }
    onkeydown?.(e);
  }}
  {...rest}
></textarea>
