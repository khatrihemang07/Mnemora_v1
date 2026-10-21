<!--
  HistorySection: what happened to a Task, in readable lines from Rust ("Due date Thu 22 Oct → Fri
  23 Oct"). The latest line shows; "Show all N" unfolds the rest. Read-only.
-->
<script lang="ts">
  import { createQuery } from '@tanstack/svelte-query';
  import type { Component } from 'svelte';
  import CalendarIcon from '~icons/lucide/calendar';
  import CircleCheck from '~icons/lucide/circle-check';
  import Circle from '~icons/lucide/circle';
  import CircleX from '~icons/lucide/circle-x';
  import Clock from '~icons/lucide/clock';
  import FilePen from '~icons/lucide/file-pen';
  import Flag from '~icons/lucide/flag';
  import FolderInput from '~icons/lucide/folder-input';
  import HistoryIcon from '~icons/lucide/history';
  import Pencil from '~icons/lucide/pencil';
  import Plus from '~icons/lucide/plus';
  import Repeat from '~icons/lucide/repeat';
  import Trash2 from '~icons/lucide/trash-2';
  import Undo2 from '~icons/lucide/undo-2';
  import { todoClock } from './clock.svelte';
  import { stampLabel } from './dates';
  import { todoPrefs } from './prefs.svelte';
  import { historyQuery } from './queries';

  let { taskId }: { taskId: string } = $props();

  // An emoji glyph is wider than its advance and runs into the word after it: give it room.
  const EMOJI = /(\p{Extended_Pictographic}\uFE0F?)/u;
  const EMOJI_ONLY = /^\p{Extended_Pictographic}\uFE0F?$/u;

  const history = createQuery(() => historyQuery(taskId));
  const lines = $derived(history.data ?? []);
  let all = $state(false);
  const shown = $derived(all ? lines : lines.slice(0, 1));

  const ICONS: Record<string, Component<{ class?: string }>> = {
    create: Plus,
    done: CircleCheck,
    reopen: Circle,
    wont: CircleX,
    delete: Trash2,
    restore: Undo2,
    list: FolderInput,
    date: CalendarIcon,
    time: Clock,
    repeat: Repeat,
    priority: Flag,
    title: Pencil,
    notes: FilePen,
  };
</script>

<section aria-label="History" class="flex flex-col gap-0.5">
  <h3 class="mb-0.5 flex items-center gap-1.5 text-[0.69rem] font-semibold tracking-[0.07em] text-faint uppercase">
    <HistoryIcon class="size-3.5" />History
    {#if lines.length > 1}
      <button
        type="button"
        onclick={() => (all = !all)}
        class="ml-auto cursor-pointer rounded-sm px-1.5 py-0.5 text-xs font-semibold tracking-normal text-hue-todo normal-case transition-colors duration-(--dur) ease-out hover:bg-hover"
      >
        {all ? 'Hide' : `Show all ${lines.length}`}
      </button>
    {/if}
  </h3>
  {#each shown as line, i (`${line.at}-${i}`)}
    {@const Icon = ICONS[line.kind] ?? Circle}
    <div class="grid grid-cols-[1.1rem_1fr_auto] items-baseline gap-x-2 px-1 text-[0.78rem] text-muted">
      <Icon class="size-3.5 translate-y-0.5 text-faint" />
      <span class="[overflow-wrap:anywhere]">
        {#each line.text.split(EMOJI) as part, i (i)}{#if EMOJI_ONLY.test(part)}<span class="mr-[0.22em]">{part}</span>{:else}{part}{/if}{/each}
      </span>
      <small class="text-[0.72rem] whitespace-nowrap text-faint">
        {stampLabel(line.at, todoClock.today, todoPrefs.timeFormat)}
      </small>
    </div>
  {:else}
    {#if !history.isPending}<p class="px-1 text-[0.78rem] text-faint">No changes yet</p>{/if}
  {/each}
</section>
