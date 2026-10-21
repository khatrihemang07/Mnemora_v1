<!--
  GroupHeader: the small uppercase label over a group of Tasks, with its count and, at the right,
  the group's actions (Overdue's "Move all"). `tone="danger"` is the red Overdue look.
-->
<script lang="ts">
  import type { Snippet } from 'svelte';

  interface Props {
    label: string;
    count: number;
    tone?: 'plain' | 'danger';
    actions?: Snippet;
  }
  let { label, count, tone = 'plain', actions }: Props = $props();
</script>

<div
  class="mt-4.5 mb-0.5 flex items-center gap-2 px-2 text-2xs font-semibold tracking-[0.07em] uppercase
    {tone === 'danger' ? 'text-danger' : 'text-muted'}"
>
  <h2 class="contents">{label}</h2>
  <span class="font-medium text-faint tabular-nums">{count}</span>
  {#if actions}<div class="ml-auto flex items-center gap-0.5 tracking-normal normal-case">{@render actions()}</div>{/if}
</div>
