<!-- The Entries and Tasks an answer cites, as chips. A chip opens a small preview: a popover on a
     desktop, a sheet on a phone, with "Open in its Module" to go to the item. -->
<script lang="ts">
  import BookOpen from '~icons/lucide/book-open';
  import SquareCheck from '~icons/lucide/square-check';
  import type { Source } from '../../lib/ipc/bindings';
  import Button from '../../lib/ui/Button.svelte';
  import Chip from '../../lib/ui/Chip.svelte';
  import Popover from '../../lib/ui/Popover.svelte';
  import Sheet from '../../lib/ui/Sheet.svelte';
  import { viewport } from '../../lib/viewport.svelte';
  import { openHref, sourceModule } from './activity';
  import { returnFocus } from './focus';

  interface Props {
    sources: readonly Source[];
  }

  let { sources }: Props = $props();
  let picked = $state<Source | null>(null);
  let previewOpen = $state(false);
  let anchor = $state<HTMLElement | null>(null);

  function show(source: Source, event: MouseEvent) {
    anchor = event.currentTarget as HTMLElement;
    picked = source;
    previewOpen = true;
  }

  function close() {
    previewOpen = false;
  }
</script>

{#if sources.length > 0}
  <div class="flex flex-col gap-2">
    <p class="text-xs font-semibold tracking-wide text-muted uppercase">Sources</p>
    <div class="flex flex-wrap gap-2">
      {#each sources as source (source.id)}
        <Chip
          icon={source.module === 'todo' ? SquareCheck : BookOpen}
          tone="warm"
          hue={source.module === 'todo' ? '--hue-todo' : '--hue-journal'}
          onclick={(event: MouseEvent) => show(source, event)}
        >
          <span class="max-w-[16rem] truncate">{source.title || sourceModule(source)}</span>
        </Chip>
      {/each}
    </div>
  </div>
{/if}

{#snippet preview(source: Source)}
  <div class="flex flex-col gap-3 p-4">
    <div class="flex flex-col gap-1">
      <p class="flex items-center gap-2 text-sm font-semibold text-ink">
        {#if source.module === 'todo'}
          <SquareCheck class="size-4 shrink-0" style="color: var(--hue-todo)" />
        {:else}
          <BookOpen class="size-4 shrink-0" style="color: var(--hue-journal)" />
        {/if}
        <span class="truncate">{source.title || sourceModule(source)}</span>
      </p>
      <p class="text-xs text-muted">{sourceModule(source)}{source.date ? ` · ${source.date}` : ''}</p>
    </div>
    <div class="flex flex-wrap gap-2">
      <Button href={openHref(source)} variant="secondary" size="sm">Open in its Module</Button>
      <Button variant="ghost" size="sm" onclick={close}>Close</Button>
    </div>
  </div>
{/snippet}

{#if picked}
  {#if viewport.isPhone}
    <Sheet open={previewOpen} title="Source" onclose={close} closable restoreFocus={() => returnFocus(anchor)}>
      {@render preview(picked)}
    </Sheet>
  {:else}
    <Popover bind:open={previewOpen} {anchor} label="Source" restoreFocus={() => returnFocus(anchor)}>
      {@render preview(picked)}
    </Popover>
  {/if}
{/if}
