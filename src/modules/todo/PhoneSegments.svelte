<!--
  PhoneSegments: the bar pinned under the large title on a phone: Today · Upcoming · Inbox and ☰,
  which opens the Lists sheet (All, Done and every List). The segment of the current view is lit;
  on All, Done or a List it is ☰. Swiping this bar (or the title) moves between the three views, see
  TodoPage. Touch sized: 40px tall, each segment a full-height target.
-->
<script lang="ts">
  import Menu from '~icons/lucide/menu';
  import { todoNav } from './nav.svelte';
  import { VIEW_ICONS } from './view-icons';
  import { activeSegment, PHONE_SEGMENTS, smartView, viewLabel } from './views';

  let { onmore, moreOpen }: { onmore: () => void; moreOpen: boolean } = $props();

  const active = $derived(activeSegment(todoNav.view));

  const seg =
    'grid h-10 cursor-pointer place-items-center rounded-md text-sm font-medium transition-colors duration-(--dur) ease-out active:scale-[0.98]';
  const lit = 'bg-[color-mix(in_srgb,var(--hue-todo)_16%,var(--surface-2))] font-semibold text-ink';
</script>

<div
  data-view-swipe
  style:touch-action="pan-y"
  class="sticky -top-(--pad) z-20 -mx-(--pad) -mt-(--pad) mb-1 bg-bg px-(--pad) pt-[calc(var(--pad)+0.125rem)] pb-2"
>
  <div role="tablist" aria-label="Todo views" class="grid grid-cols-[1fr_1fr_1fr_3rem] gap-0.5 rounded-lg bg-surface-2 p-[3px]">
    {#each PHONE_SEGMENTS as id (id)}
      {@const Icon = VIEW_ICONS[id]}
      <button
        type="button"
        role="tab"
        aria-selected={active === id}
        onclick={() => todoNav.go(smartView(id))}
        class="{seg} {active === id ? lit : 'text-muted'}"
      >
        <span class="flex items-center gap-1.5"><Icon class="size-4" />{viewLabel(id)}</span>
      </button>
    {/each}
    <button
      type="button"
      role="tab"
      aria-selected={active === 'more'}
      aria-haspopup="dialog"
      aria-expanded={moreOpen}
      aria-label="All, Done and Lists"
      onclick={onmore}
      class="{seg} {active === 'more' ? lit : 'text-muted'}"
    >
      <Menu class="size-5" />
    </button>
  </div>
</div>
