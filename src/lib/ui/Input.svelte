<!-- Input: single-line text field. Bind `value`. `label` is the accessible name. -->
<script lang="ts">
  import type { HTMLInputAttributes } from 'svelte/elements';
  interface Props extends Omit<HTMLInputAttributes, 'value'> {
    value: string;
    label: string;
    invalid?: boolean;
  }
  let { value = $bindable(), label, invalid = false, class: cls = '', ...rest }: Props = $props();
</script>

<input
  bind:value
  aria-label={label}
  aria-invalid={invalid}
  class="selectable h-(--row) min-h-8 w-full rounded-sm border bg-surface px-3 text-ink placeholder:text-faint
    transition-colors duration-(--dur) ease-out {invalid ? 'border-danger' : 'border-line focus:border-accent'} {cls}"
  {...rest}
/>
