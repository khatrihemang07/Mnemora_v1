<!--
  DayHeading: the serif heading above a day's rows, as Todo's Done has it: the day's name (Today,
  Yesterday, the weekday) in the display font, its small date beside it and, optionally, a count at
  the right. `first` trims the space above the first heading of a list.
-->
<script lang="ts">
  interface Props {
    name: string;
    small: string;
    count?: number;
    first?: boolean;
  }
  let { name, small, count, first = false }: Props = $props();
</script>

<div
  class="mb-1 flex items-baseline gap-2 border-b border-line px-2 pb-1.5 {first ? 'mt-1' : 'mt-5.5'}"
>
  <h2 class="font-display text-lg" style="font-weight: var(--display-weight)">{name}</h2>
  <small class="text-xs text-faint">{small}</small>
  {#if count !== undefined}
    <small class="ml-auto text-xs text-faint tabular-nums">{count}</small>
  {/if}
</div>
