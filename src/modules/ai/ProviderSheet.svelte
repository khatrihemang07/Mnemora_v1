<!--
  ProviderSheet: adds a Provider (kind first, then only that kind's fields) or edits one (name,
  address, a Test connection button and, for a kind that takes one, a write-only key). Deleting asks
  first. Adding a Provider tests its connection straight after it is saved, since the address is
  only then known to the device. Centred on a desktop, a bottom sheet on a phone.
-->
<script lang="ts">
  import Plug from '~icons/lucide/plug';
  import Trash2 from '~icons/lucide/trash-2';
  import { type ConnectionTest, commands, type ProviderKind, type ProviderView } from '../../lib/ipc/bindings';
  import { queryClient } from '../../lib/ipc/query-client';
  import { unwrap } from '../../lib/ipc/unwrap';
  import Button from '../../lib/ui/Button.svelte';
  import Dialog from '../../lib/ui/Dialog.svelte';
  import ErrorBanner from '../../lib/ui/ErrorBanner.svelte';
  import Input from '../../lib/ui/Input.svelte';
  import Sheet from '../../lib/ui/Sheet.svelte';
  import { notify } from '../../lib/ui/toast';
  import { aiKeys } from './queries';

  interface Props {
    open: boolean;
    /** The Provider to edit, or none to add one. */
    provider: ProviderView | null;
    onclose: () => void;
  }

  let { open, provider, onclose }: Props = $props();

  /** The kinds a Provider can be added as. Each kind has fixed behaviour; more arrive with their adapters. */
  const KINDS: { id: ProviderKind; name: string; description: string; address: string }[] = [
    {
      id: 'wrapper',
      name: 'Local AI Wrapper',
      description: 'Your localAIWrapper on this machine or your network. No key.',
      address: 'http://127.0.0.1:8080/v1',
    },
  ];

  /** The Provider as saved: set after an add, so the sheet carries on editing it. */
  let saved = $state<ProviderView | null>(null);
  const current = $derived(saved ?? provider);
  let step = $state<'kind' | 'form'>('form');
  let name = $state('');
  let address = $state('');
  let keyDraft = $state('');
  let working = $state(false);
  let error = $state<unknown>(null);
  let test = $state<ConnectionTest | null>(null);
  let confirmDelete = $state(false);

  // Each opening starts afresh: a new Provider from the kind step, an existing one from its details.
  $effect(() => {
    if (!open) return;
    saved = null;
    error = null;
    test = null;
    keyDraft = '';
    step = provider ? 'form' : 'kind';
    name = provider?.name ?? '';
    address = provider?.address ?? KINDS[0]?.address ?? '';
  });

  const title = $derived(current ? 'Provider' : 'Add a Provider');
  const kindName = $derived(
    KINDS.find((kind) => kind.id === (current?.kind ?? 'wrapper'))?.name ?? 'Provider',
  );

  function chooseKind(kind: (typeof KINDS)[number]) {
    name = kind.name;
    address = kind.address;
    step = 'form';
  }

  async function refresh() {
    await queryClient.invalidateQueries({ queryKey: aiKeys.providers });
    await queryClient.invalidateQueries({ queryKey: aiKeys.usage });
  }

  /** Saves the form: adds the Provider and tests it, or updates the one being edited. */
  async function save() {
    working = true;
    error = null;
    try {
      if (current) {
        saved = await unwrap(commands.aiProviderUpdate(current.id, { name, address }));
        notify('Provider saved');
      } else {
        saved = await unwrap(commands.aiProviderCreate({ kind: 'wrapper', name, address }));
        notify(`Added ${saved.name}`);
        test = await unwrap(commands.aiProviderTest(saved.id));
      }
      await refresh();
      await queryClient.invalidateQueries({ queryKey: aiKeys.models(saved.id) });
    } catch (caught) {
      error = caught;
    } finally {
      working = false;
    }
  }

  async function runTest() {
    if (!current) return;
    working = true;
    error = null;
    try {
      test = await unwrap(commands.aiProviderTest(current.id));
    } catch (caught) {
      error = caught;
    } finally {
      working = false;
    }
  }

  /** Saves a write-only key. The field is cleared at once; the key is never shown again. */
  async function saveKey() {
    if (!current || keyDraft.trim() === '') return;
    working = true;
    error = null;
    try {
      saved = await unwrap(commands.aiProviderKeySet(current.id, keyDraft));
      keyDraft = '';
      await refresh();
    } catch (caught) {
      error = caught;
    } finally {
      working = false;
    }
  }

  async function clearKey() {
    if (!current) return;
    working = true;
    error = null;
    try {
      saved = await unwrap(commands.aiProviderKeyClear(current.id));
      await refresh();
    } catch (caught) {
      error = caught;
    } finally {
      working = false;
    }
  }

  async function remove() {
    if (!current) return;
    confirmDelete = false;
    working = true;
    try {
      await unwrap(commands.aiProviderDelete(current.id));
      await refresh();
      notify(`Deleted ${current.name}`);
      onclose();
    } catch (caught) {
      error = caught;
    } finally {
      working = false;
    }
  }

</script>

<Sheet {open} {title} {onclose} centered closable>
  {#if step === 'kind'}
    <div class="flex flex-col gap-3">
      <p class="text-sm text-muted">Choose the kind first. Each kind asks only for what it needs.</p>
      {#each KINDS as kind (kind.id)}
        <button
          type="button"
          class="flex w-full cursor-pointer items-start gap-3 rounded-md border border-line bg-surface p-(--pad) text-left transition-colors duration-(--dur) ease-out hover:border-faint"
          onclick={() => chooseKind(kind)}
        >
          <span class="mt-0.5" style="color: var(--hue-ai)"><Plug class="size-5" /></span>
          <span class="min-w-0">
            <span class="block font-medium text-ink">{kind.name}</span>
            <span class="block text-sm text-muted">{kind.description}</span>
          </span>
        </button>
      {/each}
    </div>
  {:else}
    <form
      class="flex flex-col gap-4"
      onsubmit={(event) => {
        event.preventDefault();
        if (!working) void save();
      }}
    >
      <p class="text-sm text-muted">{kindName}</p>
      <label class="flex flex-col gap-1.5">
        <span class="text-sm font-medium text-ink">Name</span>
        <Input bind:value={name} label="Name" maxlength={40} autocomplete="off" />
      </label>
      <label class="flex flex-col gap-1.5">
        <span class="text-sm font-medium text-ink">Address</span>
        <Input bind:value={address} label="Address" autocapitalize="off" autocomplete="off" spellcheck="false" />
        <span class="text-xs text-faint">
          Plain http works only on this machine, your local network and Tailscale. Other addresses use https.
        </span>
      </label>

      {#if current && current.key_state !== 'not_needed'}
        <div class="flex flex-col gap-2 rounded-md border border-line p-(--pad)">
          <span class="text-sm font-medium text-ink">API key</span>
          {#if current.key_state === 'saved'}
            <div class="flex items-center justify-between gap-2 text-sm">
              <span class="text-ink">Key saved on this device</span>
              <span class="flex gap-2">
                <Button size="sm" variant="ghost" onclick={() => void saveKey()} disabled={keyDraft.trim() === ''}>Replace</Button>
                <Button size="sm" variant="ghost" onclick={() => void clearKey()}>Remove</Button>
              </span>
            </div>
          {/if}
          <div class="flex items-center gap-2">
            <Input
              type="password"
              bind:value={keyDraft}
              label={current.key_state === 'saved' ? 'New key' : 'API key'}
              placeholder={current.key_state === 'saved' ? 'Paste a new key to replace it' : 'Paste the key'}
              autocomplete="off"
              spellcheck={false}
            />
            <Button size="sm" variant="secondary" onclick={() => void saveKey()} disabled={keyDraft.trim() === '' || working}>
              Save key
            </Button>
          </div>
          <span class="text-xs text-faint">Write-only: hidden as you type and never shown again. Stored on this device only.</span>
        </div>
      {:else if current}
        <p class="text-sm text-muted">No key needed for this kind.</p>
      {/if}

      {#if current}
        <div class="flex flex-wrap items-center gap-3">
          <Button variant="secondary" onclick={() => void runTest()} disabled={working}>
            <Plug class="size-4" /> Test connection
          </Button>
          {#if test}
            <span class="text-sm {test.connected ? 'text-success' : 'text-danger'}" role="status">{test.message}</span>
          {/if}
        </div>
      {/if}

      {#if error}
        <ErrorBanner error={error} />
      {/if}

      <div class="flex flex-wrap items-center justify-between gap-2 pt-1">
        {#if current}
          <Button variant="danger" onclick={() => (confirmDelete = true)} disabled={working}>
            <Trash2 class="size-4" /> Delete
          </Button>
        {:else}
          <span></span>
        {/if}
        <div class="flex gap-2">
          {#if step === 'form' && !provider}
            <Button variant="ghost" onclick={() => (step = 'kind')}>Back</Button>
          {/if}
          <Button type="submit" variant="primary" disabled={working || name.trim() === '' || address.trim() === ''}>
            {current ? 'Save changes' : 'Add Provider'}
          </Button>
        </div>
      </div>
    </form>
  {/if}
</Sheet>

<Dialog
  open={confirmDelete}
  title="Delete this Provider?"
  description="Conversations that used it keep their text. Its key is removed from this device."
  onclose={() => (confirmDelete = false)}
  overSheet
>
  {#snippet actions()}
    <Button variant="ghost" onclick={() => (confirmDelete = false)}>Cancel</Button>
    <Button variant="danger" onclick={() => void remove()}>Delete</Button>
  {/snippet}
</Dialog>
