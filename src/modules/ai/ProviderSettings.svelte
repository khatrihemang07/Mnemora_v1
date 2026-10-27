<!--
  ProviderSettings: the AI section of Settings. Providers are compact rows in one Card, each with a
  status chip; a row opens its details, and the last row adds one. Changes save at once (Providers
  belong to this device), so the section has no Save bar.
-->
<script lang="ts">
  import { createQuery } from '@tanstack/svelte-query';
  import Plus from '~icons/lucide/plus';
  import Plug from '~icons/lucide/plug';
  import type { ProviderView } from '../../lib/ipc/bindings';
  import Button from '../../lib/ui/Button.svelte';
  import Card from '../../lib/ui/Card.svelte';
  import SettingSection from '../../lib/ui/SettingSection.svelte';
  import ErrorBanner from '../../lib/ui/ErrorBanner.svelte';
  import ProviderSheet from './ProviderSheet.svelte';
  import { providersOptions } from './queries';

  const providers = createQuery(() => providersOptions);
  let sheetOpen = $state(false);
  let editing = $state<ProviderView | null>(null);

  function add() {
    editing = null;
    sheetOpen = true;
  }

  function edit(provider: ProviderView) {
    editing = provider;
    sheetOpen = true;
  }

  const CHIP: Record<ProviderView['key_state'], { label: string; tone: string }> = {
    not_needed: { label: 'No key needed', tone: 'text-muted' },
    saved: { label: 'Key saved', tone: 'text-success' },
    missing: { label: 'Needs a key', tone: 'text-warning' },
  };
</script>

<SettingSection
  title="Providers"
  description="Where answers come from. Providers belong to this device; keys stay on it and never sync."
>
  {#if providers.isError}
    <ErrorBanner error={providers.error} onretry={() => void providers.refetch()} />
  {:else}
    <Card padded={false}>
      {#each providers.data ?? [] as provider (provider.id)}
        <button
          type="button"
          class="flex w-full cursor-pointer items-center gap-3 border-b border-line px-(--pad) py-3 text-left transition-colors duration-(--dur) ease-out first:rounded-t-md hover:bg-hover"
          onclick={() => edit(provider)}
        >
          <span class="flex size-8 shrink-0 items-center justify-center rounded-sm" style="background: color-mix(in oklch, var(--hue-ai) 16%, transparent); color: var(--hue-ai)">
            <Plug class="size-4" />
          </span>
          <span class="min-w-0 flex-1">
            <span class="block truncate font-medium text-ink">{provider.name}</span>
            <span class="block truncate text-sm text-muted">{provider.address}</span>
          </span>
          <span class="shrink-0 text-xs font-semibold {CHIP[provider.key_state].tone}">
            {CHIP[provider.key_state].label}
          </span>
        </button>
      {:else}
        <p class="px-(--pad) py-3 text-sm text-muted">No Providers yet. Add one to start asking questions.</p>
      {/each}
      <div class="flex justify-start p-(--pad)">
        <Button variant="secondary" onclick={add}><Plus class="size-4" /> Add a Provider</Button>
      </div>
    </Card>
  {/if}
  <p class="mt-3 text-sm text-muted">
    Answers use your Journal and Tasks, read only, and only while those Modules are on.
  </p>
</SettingSection>

<ProviderSheet open={sheetOpen} provider={editing} onclose={() => (sheetOpen = false)} />
