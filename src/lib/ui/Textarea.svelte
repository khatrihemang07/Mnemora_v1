<!--
  Textarea: multi-line text field that grows with its content up to a cap, then scrolls.
  Bind `value`; bind `el` to focus it. `label` is the accessible name.
-->
<script lang="ts">
  import type { HTMLTextareaAttributes } from 'svelte/elements';
  interface Props extends Omit<HTMLTextareaAttributes, 'value'> {
    value: string;
    label: string;
    el?: HTMLTextAreaElement | null;
  }
  let { value = $bindable(), label, el = $bindable(null), class: cls = '', ...rest }: Props =
    $props();

  // Re-measure whenever the text changes, including programmatic changes such as clearing it.
  $effect(() => {
    void value;
    if (!el) return;
    el.style.height = 'auto';
    el.style.height = `${el.scrollHeight}px`;
  });
</script>

<textarea
  bind:this={el}
  bind:value
  rows="1"
  aria-label={label}
  class="selectable block w-full resize-none bg-transparent text-ink placeholder:text-faint {cls}"
  {...rest}
></textarea>
