<!--
  Tabs: underline tabs for the pages of one Module (Sync: Devices, Conflicts, Activity). Each tab is
  meant to be a route: `value` is the tab on show and `onchange` navigates, so Back returns to the
  previous tab. The active tab carries a 2px underline in the Module `hue` that grows in with
  `transform`; arrow keys move between tabs, Home and End jump. A `count` adds a filled pill, in the
  hue (`onHue` is the ink on it). `children` is the panel of the tab on show. To pick a value rather
  than a page use SegmentedControl.
-->
<script lang="ts" generics="T extends string">
  import { Tabs } from 'bits-ui';
  import type { Snippet } from 'svelte';

  interface Tab {
    value: T;
    label: string;
    /** A number shown after the label; nothing at zero or when absent. */
    count?: number;
  }
  interface Props {
    value: T;
    tabs: Tab[];
    /** Accessible name of the tab list, e.g. "Sync pages". */
    label: string;
    onchange: (value: T) => void;
    /** A Module hue token name (`--hue-sync`) for the underline and counts; the brand accent by default. */
    hue?: string;
    /** The matching on-colour token name (`--on-hue-sync`). */
    onHue?: string;
    children?: Snippet;
  }
  let { value, tabs, label, onchange, hue, onHue, children }: Props = $props();
</script>

<Tabs.Root
  {value}
  onValueChange={(v) => onchange(v as T)}
  class="min-w-0"
  style="--tab-hue: var({hue ?? '--accent'}); --tab-on: var({onHue ?? '--on-accent'})"
>
  <Tabs.List aria-label={label} class="mb-4 flex gap-5 overflow-x-auto border-b border-line">
    {#each tabs as t (t.value)}
      <Tabs.Trigger
        value={t.value}
        class="relative -mb-px inline-flex h-(--row) min-h-9 shrink-0 cursor-pointer items-center gap-1.5 px-px text-sm whitespace-nowrap text-muted select-none
          transition-colors duration-(--dur) ease-out hover:text-ink
          after:absolute after:inset-x-0 after:bottom-0 after:h-0.5 after:origin-left after:scale-x-0 after:rounded-full after:bg-(--tab-hue)
          after:transition-transform after:duration-(--dur) after:ease-out
          data-[state=active]:font-semibold data-[state=active]:text-ink data-[state=active]:after:scale-x-100"
      >
        {t.label}
        {#if t.count}
          <span
            class="grid h-4 min-w-4 place-items-center rounded-full bg-(--tab-hue) px-1.5 text-2xs font-semibold text-(--tab-on) tabular-nums"
          >
            {t.count}
          </span>
        {/if}
      </Tabs.Trigger>
    {/each}
  </Tabs.List>
  {#if children}
    <Tabs.Content {value} class="min-w-0">{@render children()}</Tabs.Content>
  {/if}
</Tabs.Root>
