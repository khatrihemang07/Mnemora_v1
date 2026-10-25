<!--
  ModelPicker: the Model a question is asked with. A popover next to the Model pill on a desktop, a
  bottom sheet on a phone. Recent and Most used come first from this device's usage. The full list of
  a Provider is fetched only when the person chooses Browse all or types in the search box, and is
  kept for the session. A typed id is accepted even when the Provider does not list it.
-->
<script lang="ts">
  import { createQuery } from '@tanstack/svelte-query';
  import ListFilter from '~icons/lucide/list-filter';
  import Check from '~icons/lucide/check';
  import type { ProviderView } from '../../lib/ipc/bindings';
  import Button from '../../lib/ui/Button.svelte';
  import ErrorBanner from '../../lib/ui/ErrorBanner.svelte';
  import Input from '../../lib/ui/Input.svelte';
  import Popover from '../../lib/ui/Popover.svelte';
  import SegmentedControl from '../../lib/ui/SegmentedControl.svelte';
  import Sheet from '../../lib/ui/Sheet.svelte';
  import { viewport } from '../../lib/viewport.svelte';
  import type { Choice } from './choice';
  import { matchingModels, typedModelId, usesOf } from './model-picker';
  import { modelsOptions, providersOptions, usageOptions } from './queries';

  interface Props {
    open: boolean;
    /** The pill the popover hangs off (desktop). */
    anchor: HTMLElement | null;
    /** The Model in use now, if any. */
    choice: Choice | null;
    onpick: (choice: Choice) => void;
  }

  let { open = $bindable(), anchor, choice, onpick }: Props = $props();

  const providers = createQuery(() => providersOptions);
  const usage = createQuery(() => usageOptions);

  /** The Provider the list is for: the one in use, else the first. */
  let providerId = $state<string | null>(null);
  let query = $state('');
  let browsing = $state(false);

  // Each opening starts on the Provider in use, with the search cleared.
  $effect(() => {
    if (!open) return;
    providerId = choice?.providerId ?? providers.data?.[0]?.id ?? null;
    query = '';
    browsing = false;
  });

  const list = $derived(providers.data ?? []);
  const active = $derived<ProviderView | undefined>(list.find((p) => p.id === providerId) ?? list[0]);
  const typed = $derived(typedModelId(query));
  // The Provider's list is asked for only when browsing or when something is typed to search.
  const searching = $derived(browsing || typed !== null);
  const models = createQuery(() => ({
    ...modelsOptions(active?.id ?? ''),
    enabled: searching && active !== undefined && open,
  }));
  const shown = $derived(matchingModels(models.data ?? [], query));
  const recent = $derived(usesOf(usage.data?.recent ?? [], active?.id ?? ''));
  const mostUsed = $derived(usesOf(usage.data?.most_used ?? [], active?.id ?? ''));

  function pick(model: string) {
    if (!active) return;
    onpick({ providerId: active.id, model });
    open = false;
  }

  function isCurrent(model: string): boolean {
    return choice?.providerId === active?.id && choice?.model === model;
  }

  const providerOptions = $derived(list.map((p) => ({ value: p.id, label: p.name })));
</script>

{#snippet body()}
  <div class="flex max-h-[min(70dvh,30rem)] flex-col gap-3 overflow-y-auto p-3">
    {#if list.length > 1 && providerId !== null}
      <SegmentedControl
        label="Provider"
        value={providerId}
        options={providerOptions}
        onchange={(value) => {
          providerId = value;
          browsing = false;
        }}
      />
    {/if}

    <Input bind:value={query} label="Search or type a Model id" placeholder="Search or type a Model id" />

    {#if active}
      {#if !searching}
        {#if recent.length > 0}
          <div class="text-xs font-semibold text-faint">Recent</div>
          {#each recent as use (use.model)}
            <button type="button" class="flex items-center justify-between gap-2 rounded-sm px-2 py-2 text-left text-sm text-ink hover:bg-hover" onclick={() => pick(use.model)}>
              <span class="truncate">{use.model}</span>
              {#if isCurrent(use.model)}<Check class="size-4 shrink-0" />{/if}
            </button>
          {/each}
        {/if}
        {#if mostUsed.length > 0}
          <div class="text-xs font-semibold text-faint">Most used</div>
          {#each mostUsed as use (use.model)}
            <button type="button" class="flex items-center justify-between gap-2 rounded-sm px-2 py-2 text-left text-sm text-ink hover:bg-hover" onclick={() => pick(use.model)}>
              <span class="truncate">{use.model}</span>
              {#if isCurrent(use.model)}<Check class="size-4 shrink-0" />{/if}
            </button>
          {/each}
        {/if}
        {#if recent.length === 0 && mostUsed.length === 0}
          <p class="px-2 text-sm text-muted">No Models used yet. Browse this Provider’s list or type an id.</p>
        {/if}
        <Button variant="ghost" onclick={() => (browsing = true)}>
          <ListFilter class="size-4" /> Browse all models
        </Button>
      {:else}
        {#if typed !== null}
          <button type="button" class="rounded-sm px-2 py-2 text-left text-sm text-ink hover:bg-hover" onclick={() => pick(typed)}>
            Use “{typed}”
          </button>
        {/if}
        {#if models.isError}
          <ErrorBanner error={models.error} message={models.error instanceof Error ? models.error.message : undefined} onretry={() => void models.refetch()} />
        {:else if models.isPending && browsing}
          <p class="px-2 text-sm text-muted">Loading the list from {active.name}…</p>
        {:else}
          {#each shown ?? [] as model (model)}
            <button type="button" class="flex items-center justify-between gap-2 rounded-sm px-2 py-2 text-left text-sm text-ink hover:bg-hover" onclick={() => pick(model)}>
              <span class="truncate">{model}</span>
              {#if isCurrent(model)}<Check class="size-4 shrink-0" />{/if}
            </button>
          {:else}
            <p class="px-2 text-sm text-muted">No Model matches. Type its id to use it anyway.</p>
          {/each}
        {/if}
        {#if browsing}
          <Button variant="ghost" onclick={() => (browsing = false)}>Back to recent</Button>
        {/if}
      {/if}
    {:else}
      <p class="px-2 text-sm text-muted">Add a Provider first.</p>
    {/if}
  </div>
{/snippet}

{#if viewport.isPhone}
  <Sheet {open} title="Choose a Model" onclose={() => (open = false)} closable>
    {@render body()}
  </Sheet>
{:else}
  <Popover bind:open {anchor} label="Choose a Model" flush>
    {@render body()}
  </Popover>
{/if}
