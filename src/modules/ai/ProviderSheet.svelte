<!--
  ProviderSheet: adds a Provider (the kind first, then only that kind's fields) or edits one (name,
  address, a Test connection button, the paid switch for OpenRouter and, for a kind that takes one, a
  write-only key). Deleting asks first. A keyless kind is tested straight after it is added, the result
  is a toast and the sheet closes. A kind that takes a key stays open after it is added, on its key
  section, because a key can only be saved once the Provider exists and a test would fail without one.
  Saving an edit closes the sheet too. A validation error is shown once, under the field, with no Copy
  details. Centred on a desktop, a bottom sheet on a phone.
-->
<script lang="ts">
  import type { Component } from 'svelte';
  import Plug from '~icons/lucide/plug';
  import Route from '~icons/lucide/route';
  import Server from '~icons/lucide/server';
  import Sparkles from '~icons/lucide/sparkles';
  import Trash2 from '~icons/lucide/trash-2';
  import { type ConnectionTest, commands, type ProviderKind, type ProviderView } from '../../lib/ipc/bindings';
  import { queryClient } from '../../lib/ipc/query-client';
  import { IpcError, unwrap } from '../../lib/ipc/unwrap';
  import Button from '../../lib/ui/Button.svelte';
  import Dialog from '../../lib/ui/Dialog.svelte';
  import ErrorBanner from '../../lib/ui/ErrorBanner.svelte';
  import Input from '../../lib/ui/Input.svelte';
  import Sheet from '../../lib/ui/Sheet.svelte';
  import Switch from '../../lib/ui/Switch.svelte';
  import { notify } from '../../lib/ui/toast';
  import { returnFocus } from './focus';
  import { aiKeys } from './queries';
  import {
    addressFor,
    createRequest,
    KINDS,
    type KindInfo,
    keyHint,
    kindOf,
    PAID_SWITCH,
    updateRequest,
  } from './provider-kinds';

  interface Props {
    open: boolean;
    /** The Provider to edit, or none to add one. */
    provider: ProviderView | null;
    onclose: () => void;
    /** Where focus goes when the sheet closes: the control that opened it. */
    restoreFocus?: () => void;
  }

  let { open, provider, onclose, restoreFocus }: Props = $props();

  const ICONS: Record<ProviderKind, Component<{ class?: string }>> = {
    wrapper: Plug,
    openai_compatible: Server,
    anthropic: Sparkles,
    openrouter: Route,
  };

  /** The Provider as last changed here (a key set or cleared, or just added), so what is shown is current. */
  let saved = $state<ProviderView | null>(null);
  const current = $derived(saved ?? provider);
  /** The kind picked on the kind step for a new Provider. */
  let chosen = $state<ProviderKind>('wrapper');
  const info = $derived<KindInfo>(kindOf(current?.kind ?? chosen));
  let step = $state<'kind' | 'form'>('form');
  let name = $state('');
  let address = $state('');
  let allowPaid = $state(false);
  let keyDraft = $state('');
  /** A saved key being replaced: the masked field is open again. */
  let replacing = $state(false);
  let working = $state(false);
  let error = $state<unknown>(null);
  /** A validation message for the address or the key, shown once under its field. */
  let addressIssue = $state<string | null>(null);
  let keyIssue = $state<string | null>(null);
  let test = $state<ConnectionTest | null>(null);
  let confirmDelete = $state(false);
  /** The Delete button that asked for the confirmation: focus returns to it if the sheet is still open. */
  let deleteOpener = $state<HTMLElement | null>(null);

  /** After a delete the sheet has closed, so focus goes to what opened the sheet instead. */
  function restoreAfterConfirm() {
    if (deleteOpener?.isConnected) deleteOpener.focus();
    else restoreFocus?.();
  }

  // Each opening starts afresh: a new Provider from the kind step, an existing one from its details.
  $effect(() => {
    if (!open) return;
    saved = null;
    error = null;
    addressIssue = null;
    keyIssue = null;
    test = null;
    keyDraft = '';
    replacing = false;
    step = provider ? 'form' : 'kind';
    chosen = 'wrapper';
    name = provider?.name ?? '';
    address = provider?.address ?? kindOf('wrapper').address;
    allowPaid = provider?.allow_paid ?? false;
  });

  const title = $derived(current ? 'Provider' : 'Add a Provider');
  const effectiveAddress = $derived(addressFor(info, address));
  const canSave = $derived(!working && name.trim() !== '' && effectiveAddress !== '');

  function chooseKind(kind: KindInfo) {
    chosen = kind.id;
    name = kind.name;
    address = kind.address;
    allowPaid = false;
    step = 'form';
  }

  async function refresh() {
    await queryClient.invalidateQueries({ queryKey: aiKeys.providers });
    await queryClient.invalidateQueries({ queryKey: aiKeys.usage });
  }

  function isValidation(caught: unknown): caught is IpcError {
    return caught instanceof IpcError && caught.code === 'validation';
  }

  /** Tests a keyless Provider just added, and says the outcome in one line for the toast. */
  async function testAfterAdd(added: ProviderView): Promise<string> {
    try {
      const result = await unwrap(commands.aiProviderTest(added.id));
      return `Added ${added.name}. ${result.message}`;
    } catch {
      return `Added ${added.name}, but its connection could not be tested.`;
    }
  }

  /** Saves the form: adds the Provider or updates the one being edited. */
  async function save() {
    working = true;
    error = null;
    addressIssue = null;
    try {
      if (current) {
        await unwrap(commands.aiProviderUpdate(current.id, updateRequest(info, name, address, allowPaid)));
        await refresh();
        notify('Provider saved');
        onclose();
        return;
      }
      const added = await unwrap(commands.aiProviderCreate(createRequest(info, name, address)));
      // Saved already, so a failure below leaves the sheet on this Provider rather than a second add.
      saved = added;
      if (info.paid && allowPaid) {
        // Create ignores the switch, so it is sent as an update straight after.
        saved = await unwrap(commands.aiProviderUpdate(added.id, updateRequest(info, name, address, true)));
      }
      await refresh();
      await queryClient.invalidateQueries({ queryKey: aiKeys.models(added.id) });
      if (info.key !== 'none') {
        notify(
          info.key === 'required'
            ? `Added ${added.name}. Save its key to start asking.`
            : `Added ${added.name}. Save its key below if it needs one.`,
        );
        return;
      }
      notify(await testAfterAdd(added));
      onclose();
    } catch (caught) {
      if (isValidation(caught)) addressIssue = caught.message;
      else error = caught;
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
    keyIssue = null;
    try {
      saved = await unwrap(commands.aiProviderKeySet(current.id, keyDraft));
      keyDraft = '';
      replacing = false;
      await refresh();
    } catch (caught) {
      if (isValidation(caught)) keyIssue = caught.message;
      else error = caught;
    } finally {
      working = false;
    }
  }

  /**
   * Enter in the key field saves the key alone. The field sits inside the form, so without this the
   * form's submit runs and saves the Provider's details and closes the sheet with no feedback.
   */
  function onKeyEnter(event: KeyboardEvent) {
    if (event.key !== 'Enter' || event.isComposing) return;
    event.preventDefault();
    void saveKey();
  }

  function startReplace() {
    replacing = true;
    keyDraft = '';
    keyIssue = null;
  }

  function cancelReplace() {
    replacing = false;
    keyDraft = '';
    keyIssue = null;
  }

  async function clearKey() {
    if (!current) return;
    working = true;
    error = null;
    try {
      saved = await unwrap(commands.aiProviderKeyClear(current.id));
      replacing = false;
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

<Sheet {open} {title} {onclose} centered closable restoreFocus={restoreFocus}>
  {#if step === 'kind'}
    <div class="flex flex-col gap-3">
      <p class="text-sm text-muted">Choose the kind first. Each kind asks only for what it needs.</p>
      {#each KINDS as kind (kind.id)}
        {@const Icon = ICONS[kind.id]}
        <button
          type="button"
          class="flex w-full cursor-pointer items-start gap-3 rounded-md border border-line bg-surface p-(--pad) text-left transition-colors duration-(--dur) ease-out hover:border-faint"
          onclick={() => chooseKind(kind)}
        >
          <span class="mt-0.5" style="color: var(--hue-ai)"><Icon class="size-5" /></span>
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
        if (canSave) void save();
      }}
    >
      <p class="text-sm text-muted">{info.name}</p>
      <label class="flex flex-col gap-1.5">
        <span class="text-sm font-medium text-ink">Name</span>
        <Input bind:value={name} label="Name" maxlength={40} autocomplete="off" />
      </label>
      <label class="flex flex-col gap-1.5">
        <span class="text-sm font-medium text-ink">Address</span>
        {#if info.fixedAddress}
          <Input value={info.address} label="Address" readonly />
        {:else}
          <Input
            bind:value={address}
            label="Address"
            placeholder={info.id === 'openai_compatible' ? 'https://api.example.com/v1' : undefined}
            autocapitalize="off"
            autocomplete="off"
            spellcheck="false"
            invalid={addressIssue !== null}
          />
        {/if}
        {#if addressIssue}
          <span class="text-xs text-danger" role="alert">{addressIssue}</span>
        {:else if info.fixedAddress}
          <span class="text-xs text-muted">Fixed for this kind.</span>
        {:else}
          <span class="text-xs text-muted">
            Plain http works only on this machine, your local network and Tailscale. Other addresses use https.
            {#if current?.key_state === 'saved' && info.key === 'optional'}
              Changing the address removes the saved key.
            {/if}
          </span>
        {/if}
      </label>

      {#if current && info.key !== 'none'}
        <div class="flex flex-col gap-2 rounded-md border border-line p-(--pad)">
          <span class="text-sm font-medium text-ink">API key{info.key === 'optional' ? ' (optional)' : ''}</span>
          {#if current.key_state === 'saved' && !replacing}
            <div class="flex items-center justify-between gap-2 text-sm">
              <span class="text-ink">Key saved</span>
              <span class="flex gap-2">
                <Button size="sm" variant="ghost" onclick={startReplace} disabled={working}>Replace</Button>
                <Button size="sm" variant="ghost" onclick={() => void clearKey()} disabled={working}>Remove</Button>
              </span>
            </div>
          {:else}
            <div class="flex items-center gap-2">
              <Input
                type="password"
                bind:value={keyDraft}
                label="API key"
                placeholder={replacing ? 'Paste a new key' : 'Paste the key'}
                onkeydown={onKeyEnter}
                autocomplete="off"
                spellcheck={false}
                invalid={keyIssue !== null}
              />
              <Button size="sm" variant="secondary" onclick={() => void saveKey()} disabled={keyDraft.trim() === '' || working}>
                Save key
              </Button>
              {#if replacing}
                <Button size="sm" variant="ghost" onclick={cancelReplace} disabled={working}>Cancel</Button>
              {/if}
            </div>
          {/if}
          {#if keyIssue}
            <span class="text-xs text-danger" role="alert">{keyIssue}</span>
          {:else if current.key_state !== 'saved' || replacing}
            <span class="text-xs text-muted">{keyHint(info.key)}</span>
          {/if}
        </div>
      {/if}

      {#if info.paid}
        <div class="flex items-start justify-between gap-3">
          <span class="flex min-w-0 flex-col gap-0.5">
            <span class="text-sm font-medium text-ink">{PAID_SWITCH.label}</span>
            <span class="text-xs text-muted">{PAID_SWITCH.hint}</span>
          </span>
          <Switch checked={allowPaid} label={PAID_SWITCH.label} disabled={working} onchange={(next) => (allowPaid = next)} />
        </div>
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
          <Button
            variant="danger"
            onclick={(event: MouseEvent) => {
              deleteOpener = event.currentTarget as HTMLElement;
              confirmDelete = true;
            }}
            disabled={working}
          >
            <Trash2 class="size-4" /> Delete
          </Button>
        {:else}
          <span></span>
        {/if}
        <div class="flex gap-2">
          {#if step === 'form' && !current}
            <Button variant="ghost" onclick={() => (step = 'kind')}>Back</Button>
          {/if}
          <Button type="submit" variant="primary" disabled={!canSave}>
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
  restoreFocus={restoreAfterConfirm}
  overSheet
>
  {#snippet actions()}
    <Button variant="ghost" onclick={() => (confirmDelete = false)}>Cancel</Button>
    <Button variant="danger" onclick={() => void remove()}>Delete</Button>
  {/snippet}
</Dialog>
