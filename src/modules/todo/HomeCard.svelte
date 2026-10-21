<!--
  HomeCard: Todo on Home. Up to five open Tasks due today, each with its diamond checkbox and
  Priority ring: ticking one right here completes it and offers Undo, like on Today. Below them
  "N overdue" in red and "+N more". Add opens the floating Quick add due today and stays on Home;
  Open goes to Todo. A day with nothing due says "Nothing due today", and, when something is
  coming, "· next: ‹Task›, ‹date›". Refreshes when any Todo write invalidates its queries.
-->
<script lang="ts">
  import { createQuery } from '@tanstack/svelte-query';
  import Plus from '~icons/lucide/plus';
  import SquareCheckBig from '~icons/lucide/square-check-big';
  import type { HomeCardProps } from '../../lib/router';
  import { clock } from '../../lib/time/clock.svelte';
  import Button from '../../lib/ui/Button.svelte';
  import Card from '../../lib/ui/Card.svelte';
  import ErrorBanner from '../../lib/ui/ErrorBanner.svelte';
  import MarkdownInline from '../../lib/ui/MarkdownInline.svelte';
  import Skeleton from '../../lib/ui/Skeleton.svelte';
  import TaskCheckbox from '../../lib/ui/TaskCheckbox.svelte';
  import { formatHm } from '../../lib/ui/when-time';
  import { todoClock } from './clock.svelte';
  import { footerParts, homeCardModel } from './counts';
  import { timePassed } from './dates';
  import { plainTitle } from './notes';
  import { todoNav } from './nav.svelte';
  import OpenStepsDialog from './OpenStepsDialog.svelte';
  import { followCoreSettings, todoPrefs } from './prefs.svelte';
  import { countsQuery } from './queries';
  import { quickAdd } from './quick-add.svelte';
  import { tickTask } from './task-actions';
  import { priorityTone } from './tasks';
  import { smartView } from './views';

  let { ctx }: HomeCardProps = $props();

  $effect(() => clock.subscribe());
  followCoreSettings();

  const counts = createQuery(() => countsQuery());
  const model = $derived(counts.data ? homeCardModel(counts.data, todoClock.today) : null);
  const footer = $derived(model ? footerParts(model) : null);

  function open() {
    todoNav.go(smartView('today'));
    ctx.navigate('/todo');
  }
</script>

<Card>
  <div class="flex items-center gap-2.5">
    <span
      class="grid size-8 place-items-center rounded-full bg-[color-mix(in_oklch,var(--hue-todo)_14%,transparent)] text-(--hue-todo)"
    >
      <SquareCheckBig class="size-4" />
    </span>
    <h2 class="font-semibold text-ink">Todo</h2>
    <div class="ml-auto flex items-center gap-1.5">
      <Button size="sm" variant="secondary" onclick={() => quickAdd.openFloating(todoClock.today)}>
        <Plus class="size-3.5" /> Add
      </Button>
      <Button size="sm" variant="ghost" onclick={open}>Open</Button>
    </div>
  </div>
  {#if counts.isError}
    <div class="mt-3"><ErrorBanner error={counts.error} onretry={() => counts.refetch()} /></div>
  {:else if model && footer}
    <div class="mt-2">
      {#each model.tasks as task (task.id)}
        <div class="flex items-start gap-2 rounded-sm px-1 py-1.5">
          <TaskCheckbox
            checked={false}
            tone={task.priority < 4 ? priorityTone(task.priority) : 'plain'}
            label="Complete {plainTitle(task.title)}"
            onchange={() => void tickTask(task)}
            class="mt-px"
          />
          <button
            type="button"
            class="min-w-0 flex-1 cursor-pointer text-left leading-[1.45] [overflow-wrap:anywhere]"
            onclick={open}
          >
            <MarkdownInline source={task.title} />
          </button>
          {#if task.due_time}
            <span
              class="mt-px shrink-0 text-xs tabular-nums {timePassed(task, todoClock.today, todoClock.hm)
                ? 'text-danger'
                : 'text-faint'}"
            >
              {formatHm(task.due_time, todoPrefs.timeFormat)}
            </span>
          {/if}
        </div>
      {/each}
      {#if model.nothing}
        <p class="px-1 py-1.5 text-sm text-faint">{model.nothing}</p>
      {/if}
      {#if footer.overdue || footer.more}
        <p class="px-1 pt-1 text-xs text-faint">
          {#if footer.overdue}<span class="text-danger">{footer.overdue}</span>{/if}
          {#if footer.overdue && footer.more} · {/if}
          {#if footer.more}{footer.more}{/if}
        </p>
      {/if}
    </div>
  {:else}
    <Skeleton class="mt-3 h-12 w-full" />
  {/if}
</Card>
<OpenStepsDialog />
