<!--
  GroupHeader: the small uppercase label over a group of Tasks, with its count and, at the right,
  the group's actions (Overdue's "Move all"). `tone="danger"` is the red Overdue look. A Priority
  group (`priority` 1 to 3) wears the flag of that Priority; `icon="calendar-off"` marks the Inbox's
  "No date, in a List".
-->
<script lang="ts">
  import type { Snippet } from 'svelte';
  import CalendarOff from '~icons/lucide/calendar-off';
  import Flag from '~icons/lucide/flag';

  interface Props {
    label: string;
    count: number;
    tone?: 'plain' | 'danger';
    priority?: number;
    icon?: 'calendar-off';
    actions?: Snippet;
  }
  let { label, count, tone = 'plain', priority, icon, actions }: Props = $props();

  const FLAG: Record<number, string> = { 1: 'text-danger', 2: 'text-warning', 3: 'text-accent' };
</script>

<div
  class="mt-4.5 mb-0.5 flex items-center gap-2 px-2 text-2xs font-semibold tracking-[0.07em] uppercase
    {tone === 'danger' ? 'text-danger' : 'text-muted'}"
>
  {#if priority !== undefined}<Flag class="-mr-0.5 size-3 {FLAG[priority] ?? ''}" />{/if}
  {#if icon === 'calendar-off'}<CalendarOff class="-mr-0.5 size-3" />{/if}
  <h2 class="contents">{label}</h2>
  <span class="font-medium text-faint tabular-nums">{count}</span>
  {#if actions}<div class="ml-auto flex items-center gap-0.5 tracking-normal normal-case">{@render actions()}</div>{/if}
</div>
