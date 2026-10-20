<!-- Select: single-choice dropdown on Bits UI. -->
<script lang="ts" generics="T extends string">
  import { Select } from 'bits-ui';
  import Check from '~icons/lucide/check';
  import ChevronDown from '~icons/lucide/chevron-down';

  interface Props {
    value: T;
    options: { value: T; label: string }[];
    label: string;
    onchange: (value: T) => void;
  }
  let { value, options, label, onchange }: Props = $props();
  const current = $derived(options.find((o) => o.value === value)?.label ?? '');
</script>

<Select.Root type="single" {value} onValueChange={(v) => v && onchange(v as T)} items={options}>
  <Select.Trigger
    aria-label={label}
    class="inline-flex h-(--row) min-h-8 min-w-36 cursor-pointer items-center justify-between gap-2 rounded-sm border border-line bg-surface px-3 text-ink
      transition-colors duration-(--dur) ease-out hover:bg-hover"
  >
    {current}
    <ChevronDown class="size-4 text-muted" />
  </Select.Trigger>
  <Select.Portal>
    <Select.Content
      sideOffset={4}
      class="z-50 min-w-(--bits-select-anchor-width) rounded-md bg-pop p-1 shadow-pop"
    >
      <Select.Viewport>
        {#each options as o (o.value)}
          <Select.Item
            value={o.value}
            label={o.label}
            class="flex h-8 cursor-pointer items-center justify-between rounded-sm px-2.5 text-ink select-none data-highlighted:bg-hover"
          >
            {#snippet children({ selected })}
              {o.label}
              {#if selected}<Check class="size-4 text-accent" />{/if}
            {/snippet}
          </Select.Item>
        {/each}
      </Select.Viewport>
    </Select.Content>
  </Select.Portal>
</Select.Root>
