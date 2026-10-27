<!--
  FailureBanner: a failed Run, shown in the answer's place. The title and actions come from the
  failure kind; the sentence is the one Rust wrote (the Provider's own text is never shown).
-->
<script lang="ts">
  import CircleAlert from '~icons/lucide/circle-alert';
  import TriangleAlert from '~icons/lucide/triangle-alert';
  import Button from '../../lib/ui/Button.svelte';
  import { type BannerAction, failureBanner } from './failure-banner';
  import type { RunFailure } from './run-state';

  interface Props {
    failure: RunFailure;
    /** The handlers for the actions this failure offers; an action without one is not shown. */
    handlers: Partial<Record<BannerAction, () => void>>;
  }

  let { failure, handlers }: Props = $props();
  const banner = $derived(failureBanner(failure.kind));
  const labels: Record<BannerAction, string> = {
    retry: 'Try again',
    change_model: 'Change Model',
    change_provider: 'Change Provider',
    branch_from_earlier: 'Branch from earlier...',
    new_conversation: 'New Conversation',
  };
  const shown = $derived(banner.actions.filter((action) => handlers[action] !== undefined));
</script>

<div
  role="alert"
  class="flex flex-wrap items-start gap-3 rounded-md border px-(--pad) py-3 text-sm"
  style={banner.tone === 'danger'
    ? 'border-color: var(--danger); background: var(--danger-soft)'
    : 'border-color: color-mix(in oklch, var(--warning) 50%, transparent); background: color-mix(in oklch, var(--warning) 16%, transparent)'}
>
  <span class="mt-0.5 shrink-0" style="color: {banner.tone === 'danger' ? 'var(--danger)' : 'var(--warning)'}">
    {#if banner.tone === 'danger'}<CircleAlert class="size-4" />{:else}<TriangleAlert class="size-4" />{/if}
  </span>
  <div class="min-w-[12rem] flex-1">
    <div class="font-semibold text-ink">{banner.title}</div>
    <p class="text-muted">{failure.message}</p>
  </div>
  {#if shown.length > 0}
    <div class="flex flex-wrap gap-2">
      {#each shown as action, index (action)}
        <Button size="sm" variant={index === 0 ? 'primary' : 'secondary'} onclick={handlers[action]}>
          {labels[action]}
        </Button>
      {/each}
    </div>
  {/if}
</div>
