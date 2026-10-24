<!--
  DeviceSheet: one paired device in a Sheet over the Devices tab (centred on a desktop, a bottom sheet on a
  phone), at `/sync?device=‹id›`. Back closes it and a link opens it. It shows where the device stands, the
  addresses that worked and when, the versions it runs, when and by whom it was paired; the optional Address
  (a MagicDNS name, host, host:port, IPv4 or [IPv6]) with Check, which dials it and says "Reached Pixel 8 in
  84 ms" or why not; and Sync now, Rename and Forget. Forget asks first: it is group-wide.
-->
<script lang="ts">
  import { createQuery } from '@tanstack/svelte-query';
  import CircleAlert from '~icons/lucide/circle-alert';
  import CircleCheck from '~icons/lucide/circle-check';
  import Laptop from '~icons/lucide/laptop';
  import Pencil from '~icons/lucide/pencil';
  import RefreshCw from '~icons/lucide/refresh-cw';
  import Smartphone from '~icons/lucide/smartphone';
  import Unlink from '~icons/lucide/unlink';
  import { commands, type CheckResult } from '../../lib/ipc/bindings';
  import { queryClient } from '../../lib/ipc/query-client';
  import { unwrap } from '../../lib/ipc/unwrap';
  import { clock } from '../../lib/time/clock.svelte';
  import Button from '../../lib/ui/Button.svelte';
  import Dialog from '../../lib/ui/Dialog.svelte';
  import ErrorBanner from '../../lib/ui/ErrorBanner.svelte';
  import Input from '../../lib/ui/Input.svelte';
  import Sheet from '../../lib/ui/Sheet.svelte';
  import Skeleton from '../../lib/ui/Skeleton.svelte';
  import { notifyError } from '../../lib/ui/toast';
  import { addressLines, deviceGone, pairedText, versionsText } from './device-detail';
  import { DeviceRename } from './device-rename.svelte';
  import { deviceSheet } from './device-sheet.svelte';
  import { devicesQuery, deviceDetailQuery, watchDevices } from './queries';
  import { statusWord, syncedText } from './peer-status';
  import { CHIP, DOT } from './peer-style';

  let { id }: { id: string } = $props();

  const detail = createQuery(() => deviceDetailQuery(id));
  const d = $derived(detail.data);

  // A link to a device that is not paired, or a device another member just forgot: the sheet has
  // nothing to show, so it closes and says why.
  $effect(() => {
    if (!deviceGone(detail.error)) return;
    deviceSheet.dismiss();
    notifyError('That device is not paired');
  });

  // Status changes while the sheet is open (a sync finishes, the device goes away) arrive as events.
  $effect(() => {
    let stop: (() => void) | undefined;
    let gone = false;
    void watchDevices().then((off) => (gone ? off() : (stop = off)));
    return () => {
      gone = true;
      stop?.();
    };
  });
  // A relative time moves on while the sheet is open.
  $effect(() => clock.subscribe());

  const Icon = $derived(d?.platform === 'phone' ? Smartphone : Laptop);
  const lines = $derived(d ? addressLines(d, clock.now) : []);
  const versions = $derived(d ? versionsText(d) : null);

  async function refresh() {
    await Promise.all([
      queryClient.invalidateQueries({ queryKey: deviceDetailQuery(id).queryKey }),
      queryClient.invalidateQueries({ queryKey: devicesQuery.queryKey }),
    ]);
  }

  // Rename: the sheet stays open and shows the new name (see device-rename.svelte.ts).
  const rename = new DeviceRename(async (name) => {
    await unwrap(commands.syncDeviceRename(id, name));
    await refresh();
  });
  let nameInput = $state<HTMLInputElement | null>(null);
  $effect(() => {
    if (rename.editing) nameInput?.focus();
  });

  // The Address: saved when you leave the field or press Enter; Check dials what is typed.
  let typed = $state<string | null>(null);
  let addressError = $state<string | null>(null);
  const addressText = $derived(typed ?? d?.typed_address ?? '');
  let checking = $state(false);
  let result = $state<CheckResult | null>(null);
  async function saveAddress() {
    if (typed === null || typed.trim() === (d?.typed_address ?? '')) return;
    try {
      await unwrap(commands.syncDeviceAddress(id, typed));
      typed = null;
      addressError = null;
      await refresh();
    } catch (e) {
      addressError = e instanceof Error ? e.message : 'Could not save the address';
    }
  }
  async function check() {
    checking = true;
    result = null;
    try {
      result = await unwrap(commands.syncDeviceCheck(id, addressText));
    } catch (e) {
      result = {
        reached: false,
        kind: 'other',
        message: e instanceof Error ? e.message : 'Could not check',
        ms: null,
      };
    } finally {
      checking = false;
    }
    await refresh();
  }

  let forgetting = $state(false);
  async function forget() {
    forgetting = false;
    try {
      await unwrap(commands.syncDeviceForget(id));
    } catch (e) {
      notifyError(e instanceof Error ? e.message : 'Could not forget the device');
      return;
    }
    deviceSheet.dismiss();
    await queryClient.invalidateQueries({ queryKey: devicesQuery.queryKey });
  }
</script>

<!-- `bare`: the name is the title (the sheet's own small label would say it twice), so the sheet lays itself out. -->
<Sheet open title={d?.name ?? 'Device'} centered bare returnFocus onclose={() => deviceSheet.close()}>
  {#if detail.isError}
    <div class="p-4"><ErrorBanner error={detail.error} onretry={() => detail.refetch()} /></div>
  {:else if d}
    <div class="flex flex-col gap-4 px-4 pt-5 pb-4" data-device={d.id}>
      <div class="flex items-center gap-3">
        <span class="relative grid size-10 shrink-0 place-items-center rounded-md bg-hue-sync-soft text-hue-sync">
          <Icon class="size-5" />
          <span
            class="absolute -right-0.5 -bottom-0.5 size-2.5 rounded-full border-2 border-surface {DOT[d.state]}"
            aria-hidden="true"
          ></span>
        </span>
        <div class="min-w-0 flex-1">
          {#if rename.editing}
            <Input
              label="Device name"
              bind:value={() => rename.text, (v) => rename.type(v)}
              invalid={rename.error !== null}
              bind:ref={nameInput}
              onblur={() => void rename.commit(d.name)}
              onkeydown={(e) => {
                if (e.key === 'Enter') {
                  e.preventDefault();
                  void rename.commit(d.name);
                } else if (e.key === 'Escape') {
                  e.stopPropagation();
                  rename.cancel();
                }
              }}
            />
            {#if rename.error}<p role="alert" class="mt-1 text-sm text-danger">{rename.error}</p>{/if}
          {:else}
            <div class="font-semibold wrap-anywhere">{d.name}</div>
            <div class="flex flex-wrap items-center gap-x-1.5 text-sm text-muted">
              <span class="rounded-full px-2 text-xs leading-5 font-semibold {CHIP[d.state]}">{statusWord(d.state)}</span>
              <span>{syncedText(d, clock.now)}</span>
            </div>
          {/if}
        </div>
      </div>

      <dl class="grid grid-cols-[auto_1fr] gap-x-4 gap-y-2 text-sm">
        <dt class="text-muted">Addresses</dt>
        <dd class="min-w-0">
          {#if lines.length === 0}
            <span class="text-muted">None yet</span>
          {:else}
            <ul class="flex flex-col gap-0.5">
              {#each lines as line (line.address)}
                <li><span class="font-mono text-xs break-all">{line.address}</span> <span class="text-faint">({line.when})</span></li>
              {/each}
            </ul>
          {/if}
        </dd>
        <dt class="text-muted">Versions</dt>
        <dd class="min-w-0">{versions ?? 'Known once it has been reached'}</dd>
        <dt class="text-muted">Paired</dt>
        <dd>{pairedText(d)}</dd>
      </dl>

      <div>
        <div class="mb-1 text-xs text-muted">
          Address <span class="text-faint">(optional, e.g. a Tailscale MagicDNS name)</span>
        </div>
        <div class="flex items-center gap-2">
          <Input
            label="Address of {d.name}"
            placeholder="pixel.tail1234.ts.net"
            class="min-w-0 flex-1"
            invalid={addressError !== null}
            bind:value={() => addressText, (v) => { typed = v; addressError = null; result = null; }}
            autocapitalize="off"
            autocorrect="off"
            spellcheck={false}
            onblur={() => void saveAddress()}
            onkeydown={(e) => e.key === 'Enter' && void saveAddress()}
          />
          <Button size="sm" disabled={checking || addressText.trim() === ''} onclick={() => void check()}>
            {checking ? 'Checking…' : 'Check'}
          </Button>
        </div>
        {#if addressError}
          <p role="alert" class="mt-1 text-sm text-danger">{addressError}</p>
        {:else if result}
          <p
            role="status"
            class="mt-1 flex items-start gap-1.5 text-xs {result.reached ? 'text-success' : 'text-danger'}"
          >
            {#if result.reached}<CircleCheck class="mt-px size-3.5 shrink-0" />{:else}<CircleAlert class="mt-px size-3.5 shrink-0" />{/if}
            <span>{result.message}</span>
          </p>
        {/if}
      </div>

      <div class="flex flex-wrap items-center gap-2">
        <Button size="sm" onclick={() => void commands.syncDeviceSyncNow(id)}>
          <RefreshCw class="size-3.5" /> Sync now
        </Button>
        <Button size="sm" variant="ghost" onclick={() => rename.start(d.name)}>
          <Pencil class="size-3.5" /> Rename
        </Button>
        <span class="flex-1"></span>
        <!-- Outline, not filled: the Dialog it opens asks first, and that one has the filled Forget. -->
        <Button
          size="sm"
          class="border-danger! text-danger!"
          onclick={() => (forgetting = true)}
        >
          <Unlink class="size-3.5" /> Forget…
        </Button>
      </div>
    </div>
  {:else}
    <Skeleton class="m-4 h-40" />
  {/if}
</Sheet>

<Dialog
  open={forgetting}
  title="Forget {d?.name ?? 'this device'}?"
  description="It won’t sync with any of your devices again. Its data stays on it. To sync it later, pair it again."
  onclose={() => (forgetting = false)}
  overSheet
>
  {#snippet actions()}
    <Button onclick={() => (forgetting = false)}>Cancel</Button>
    <Button variant="danger" onclick={() => void forget()}>Forget</Button>
  {/snippet}
</Dialog>
