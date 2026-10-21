<!--
  ModuleBadge: a Module's count (Todo's due today plus Overdue) beside its sidebar name, over its
  icon on the rail, or over its tab icon on a phone. Red when something is Overdue. Renders nothing
  for a Module without a badge or with nothing to count.
-->
<script lang="ts">
  import type { ModuleManifest } from '../lib/router';

  interface Props {
    manifest: ModuleManifest;
    /** `inline` follows the name; `corner` floats over the top right of the icon beside it. */
    placement: 'inline' | 'corner';
  }
  let { manifest, placement }: Props = $props();

  // The manifest of a row never changes, so its hook is called once, as component setup.
  // svelte-ignore state_referenced_locally
  const source = manifest.useBadge?.();
  const badge = $derived(source?.badge ?? null);
  const shown = $derived(badge && badge.count > 99 ? '99+' : String(badge?.count ?? ''));
</script>

{#if badge}
  <span
    data-badge={badge.alert ? 'alert' : 'plain'}
    aria-label="{badge.count} due or overdue"
    class="grid h-4 min-w-4 shrink-0 place-items-center rounded-full px-1 text-2xs font-semibold tabular-nums
      {badge.alert ? 'bg-danger text-on-accent' : 'bg-active text-muted'}
      {placement === 'corner' ? 'absolute -top-2 -right-3.5 ring-2 ring-side' : ''}"
  >
    {shown}
  </span>
{/if}
