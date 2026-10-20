<!--
  Textarea: multi-line text field that grows with its content up to a cap, then scrolls.
  Bind `value`; bind `el` to focus it. `label` is the accessible name. `bare` drops the field's own
  focus halo for use inside a frame that shows focus itself (the Journal composer).
-->
<script lang="ts">
  import type { HTMLTextareaAttributes } from 'svelte/elements';
  interface Props extends Omit<HTMLTextareaAttributes, 'value'> {
    value: string;
    label: string;
    el?: HTMLTextAreaElement | null;
    bare?: boolean;
  }
  let {
    value = $bindable(),
    label,
    el = $bindable(null),
    bare = false,
    class: cls = '',
    ...rest
  }: Props = $props();

  // Fit the content; scroll only past the CSS max-height, so no idle scrollbar shows.
  function fit(t: HTMLTextAreaElement) {
    t.style.height = 'auto';
    t.style.height = `${t.scrollHeight}px`;
    t.style.overflowY = t.scrollHeight > t.clientHeight + 1 ? 'auto' : 'hidden';
  }

  // Re-measure whenever the text changes, including programmatic changes such as clearing it.
  $effect(() => {
    void value;
    if (el) fit(el);
  });

  // Web fonts change line metrics after the first measure.
  $effect(() => {
    const t = el;
    if (t) void document.fonts?.ready.then(() => fit(t));
  });
</script>

<textarea
  bind:this={el}
  bind:value
  rows="1"
  aria-label={label}
  data-bare={bare || undefined}
  class="selectable block w-full resize-none overflow-hidden bg-transparent text-ink placeholder:text-faint {cls}"
  {...rest}
></textarea>
