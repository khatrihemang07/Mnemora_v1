<!--
  Input: single-line text field. Bind `value`. `label` is the accessible name. `bare` drops the frame
  (border, fill, height, focus ring) for a field inside a frame that shows focus itself. Bind `ref` to
  focus it.
-->
<script lang="ts">
  import type { HTMLInputAttributes } from 'svelte/elements';
  interface Props extends Omit<HTMLInputAttributes, 'value'> {
    value: string;
    label: string;
    invalid?: boolean;
    bare?: boolean;
    ref?: HTMLInputElement | null;
  }
  let { value = $bindable(), label, invalid = false, bare = false, ref = $bindable(null), class: cls = '', ...rest }: Props = $props();
</script>

<input
  bind:this={ref}
  bind:value
  aria-label={label}
  aria-invalid={invalid}
  data-bare={bare ? '' : undefined}
  class="selectable w-full text-ink placeholder:text-faint {bare
    ? 'bg-transparent'
    : `h-(--row) min-h-8 rounded-sm border bg-surface px-3 transition-colors duration-(--dur) ease-out ${invalid ? 'border-danger' : 'border-line focus:border-accent'}`} {cls}"
  {...rest}
/>
