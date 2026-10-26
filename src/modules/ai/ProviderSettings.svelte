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
  import SavedQuestions from './SavedQuestions.svelte';
  import { returnFocus } from './focus';
  import { providersOptions } from './queries';
  import { freeOnlyNote, keyChip } from './provider-kinds';

  const providers = createQuery(() => providersOptions);
  let sheetOpen = $state(false);
  let editing = $state<ProviderView | null>(null);
  /** The row or button that opened the sheet: focus goes back to it when the sheet closes. */
  let opener = $state<HTMLElement | null>(null);

  function add(event: MouseEvent) {
    opener = event.currentTarget as HTMLElement;
    editing = null;
    sheetOpen = true;
  }

  function edit(provider: ProviderView, event: MouseEvent) {
    opener = event.currentTarget as HTMLElement;
    editing = provider;
    sheetOpen = true;
  }

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
        {@const chip = keyChip(provider.key_state)}
        {@const note = freeOnlyNote(provider)}
        <button
          type="button"
          class="flex w-full cursor-pointer items-center gap-3 border-b border-line px-(--pad) py-3 text-left transition-colors duration-(--dur) ease-out first:rounded-t-md hover:bg-hover"
          onclick={(event: MouseEvent) => edit(provider, event)}
        >
          <span class="flex size-8 shrink-0 items-center justify-center rounded-sm" style="background: color-mix(in oklch, var(--hue-ai) 16%, transparent); color: var(--hue-ai)">
            <Plug class="size-4" />
          </span>
          <span class="min-w-0 flex-1">
            <span class="block truncate font-medium text-ink">{provider.name}</span>
            <span class="block truncate text-sm text-muted">{provider.address}</span>
            {#if note}<span class="block truncate text-xs text-muted">{note}</span>{/if}
          </span>
          {#if chip}
            <span class="inline-flex shrink-0 items-center rounded-full border border-line-strong bg-surface-2 px-2.5 py-1 text-xs font-semibold whitespace-nowrap {chip.tone === 'warn' ? 'text-warning-ink' : 'text-success'}">
              {chip.label}
            </span>
          {/if}
        </button>
      {:else}
        <p class="px-(--pad) py-3 text-sm text-muted">No Providers yet. Add one to start asking questions.</p>
      {/each}
      <div class="flex justify-start p-(--pad)">
        <Button variant="secondary" onclick={(event: MouseEvent) => add(event)}><Plus class="size-4" /> Add a Provider</Button>
      </div>
    </Card>
  {/if}
  <p class="mt-3 text-sm text-muted">
    Answers use your Journal and Tasks, read only, and only while those Modules are on.
  </p>
</SettingSection>

<div class="mt-8"><SavedQuestions /></div>

<ProviderSheet
  open={sheetOpen}
  provider={editing}
  onclose={() => (sheetOpen = false)}
  restoreFocus={() => returnFocus(opener)}
/>
