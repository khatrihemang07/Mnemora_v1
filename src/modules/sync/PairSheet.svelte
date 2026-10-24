<!--
  PairSheet: pairing, in a Sheet over the Devices tab (centred on a desktop, a bottom sheet on a
  phone). One device shows an 8-digit code with a 2-minute ring; the other types it; the first asks
  Allow or Deny inside the same window. Rust says which phase this is (`pairing.view`); the sheet only
  renders it and sends back what the person does. Every failure is a banner with what to do next.
  "Pair by address…" (typing mode, below the code boxes) asks for the other device's address too, for a
  device that is not on this Wi-Fi (Tailscale).
-->
<script lang="ts">
  import { onMount } from 'svelte';
  import CircleAlert from '~icons/lucide/circle-alert';
  import Pause from '~icons/lucide/pause';
  import Archive from '~icons/lucide/archive';
  import CircleCheck from '~icons/lucide/circle-check';
  import Laptop from '~icons/lucide/laptop';
  import Smartphone from '~icons/lucide/smartphone';
  import type { Platform } from '../../lib/ipc/bindings';
  import Button from '../../lib/ui/Button.svelte';
  import CodeInput from '../../lib/ui/CodeInput.svelte';
  import Input from '../../lib/ui/Input.svelte';
  import { clock as appClock } from '../../lib/time/clock.svelte';
  import Sheet from '../../lib/ui/Sheet.svelte';
  import CountdownRing from './CountdownRing.svelte';
  import { PAIRING_PAUSED, clock, fractionLeft, previewStats, problem, remaining } from './pair-view';
  import { pairing } from './pairing.svelte';

  // Rust sends the time left when a view arrives; the sheet counts down from there.
  const counting = $derived(pairing.view.state === 'showing' || pairing.view.state === 'asking');
  $effect(() => {
    if (!pairing.open || !counting) return;
    return appClock.subscribe();
  });
  const now = $derived(appClock.now.getTime());
  const liveLeft = $derived(remaining(pairing.view, Math.max(0, now - pairing.arrivedAt)));
  const liveLife = $derived(
    pairing.view.state === 'showing' || pairing.view.state === 'asking' ? pairing.view.life_ms : 0,
  );

  onMount(() => {
    let stop = () => {};
    let gone = false;
    void pairing.listen().then((unlisten) => {
      if (gone) unlisten();
      else stop = unlisten;
    });
    return () => {
      gone = true;
      stop();
      // Leaving the Devices page ends a pairing in progress: no code is left open behind it.
      if (pairing.open) void pairing.close();
    };
  });

  // While the sheet fades out, Rust's next view (idle, after Deny or Cancel) must not replace what it
  // shows: it keeps the last view it had when open until the close transition ends.
  let held = $state.raw(pairing.view);
  // The countdown freezes with it, so it never reads "0:00 left" for a frame while the sheet closes.
  let heldLeft = $state(0);
  let heldLife = $state(0);
  $effect.pre(() => {
    if (!pairing.open) return;
    held = pairing.view;
    heldLeft = liveLeft;
    heldLife = liveLife;
  });
  const left = $derived(pairing.open ? liveLeft : heldLeft);
  const life = $derived(pairing.open ? liveLife : heldLife);
  const view = $derived(pairing.open ? pairing.view : held);
  const byAddress = $derived(pairing.mode === 'address');
  const typing = $derived(pairing.mode !== 'show');
  const failure = $derived(view.state === 'failed' ? problem(view.error, typing) : null);
  // Which of the two fields a failure is about.
  const addressWrong = $derived(
    view.state === 'failed' &&
      (view.error.kind === 'address_not_found' || view.error.kind === 'address_unreachable'),
  );
  const wrongCode = $derived(view.state === 'failed' && view.error.kind === 'wrong_code');
  const notice = $derived(view.state === 'showing' && view.notice ? problem(view.notice, false) : null);
  const stranger = (platform: Platform) => (platform === 'phone' ? Smartphone : Laptop);
  // The one title of the step: read out by screen readers and shown as its heading.
  const title = $derived(
    view.state === 'paired'
      ? 'Paired'
      : view.state === 'preview'
        ? `Merge with ${view.name}?`
        : byAddress && view.state !== 'waiting'
        ? 'Pair by address'
        : view.state === 'searching' || (typing && view.state !== 'waiting')
          ? 'Enter a code'
          : 'Pair a device',
  );
  // "Pixel 8 (2)" was "Pixel 8" before the clash.
  const unnumbered = (name: string) => name.replace(/ \(\d+\)$/, '');
  const phrase = (n: number) => (n === 1 ? '1 try' : `${n} tries`);
</script>

{#snippet banner(title: string, fix: string)}
  <div role="alert" class="mt-3 flex items-start gap-2.5 rounded-md border border-danger bg-danger-soft px-3.5 py-2.5 text-left text-sm text-ink">
    <CircleAlert class="mt-0.5 size-4 shrink-0 text-danger" />
    <div><b class="font-semibold">{title}</b><br /><span class="text-muted">{fix}</span></div>
  </div>
{/snippet}

{#snippet heading(text: string)}
  <h2 class="mb-1.5 text-base font-semibold text-ink">{text}</h2>
{/snippet}

<Sheet open={pairing.open} {title} onclose={() => void pairing.close()} centered bare returnFocus>
  <div class="px-5 pt-6 pb-5" data-pairing={view.state}>
    {#if pairing.pausedOnMobile}
      {@render heading(title)}
      <div role="alert" class="mt-3 flex items-start gap-2.5 rounded-md border border-[color-mix(in_oklch,var(--warning)_50%,transparent)] bg-[color-mix(in_oklch,var(--warning)_16%,transparent)] px-3.5 py-2.5 text-left text-sm text-ink">
        <Pause class="mt-0.5 size-4 shrink-0 text-warning" />
        <div><b class="font-semibold">{PAIRING_PAUSED.title}</b><br /><span class="text-muted">{PAIRING_PAUSED.fix}</span></div>
      </div>
      <div class="mt-4 flex justify-end gap-2">
        <Button onclick={() => void pairing.close()}>Close</Button>
        <Button variant="primary" onclick={() => void pairing.start(pairing.mode)}>Try again</Button>
      </div>
    {:else if pairing.commandError}
      {@render heading(title)}
      {@render banner(
        pairing.commandError instanceof Error ? pairing.commandError.message : 'Pairing did not start.',
        'Try again in a moment.',
      )}
      <div class="mt-4 flex justify-end gap-2">
        <Button onclick={() => void pairing.close()}>Close</Button>
        <Button variant="primary" onclick={() => void pairing.start(pairing.mode)}>Try again</Button>
      </div>
    {:else if view.state === 'paired'}
      <div class="flex flex-col items-center gap-1 text-center">
        <CircleCheck class="size-9 text-success" />
        <h2 class="text-base font-semibold text-ink">Paired with {view.name}</h2>
        {#if view.renamed}
          <p class="text-sm text-muted">
            You already had a "{unnumbered(view.name)}", so this one is called <b>{view.name}</b>. Rename it any time.
          </p>
        {/if}
        {#if view.also_joined.length > 0}
          <p class="text-sm text-ink">
            Also joined: {#each view.also_joined as joined, i (joined)}<b>{joined}</b>{i < view.also_joined.length - 1 ? ', ' : ''}{/each}
          </p>
        {/if}
        <p class="text-sm text-muted">Syncing now. You can keep using the app.</p>
        <Button variant="primary" class="mt-3" onclick={() => void pairing.close()}>Done</Button>
      </div>
    {:else if view.state === 'asking'}
      {@const Icon = stranger(view.platform)}
      {@render heading(title)}
      <div class="flex items-center gap-3">
        <span class="grid size-10 shrink-0 place-items-center rounded-md bg-hue-sync-soft text-hue-sync"><Icon class="size-5" /></span>
        <div class="min-w-0">
          <div class="truncate font-semibold text-ink">{view.name}</div>
          <div class="text-sm text-muted">wants to pair with this device</div>
        </div>
      </div>
      <div class="mt-4 flex flex-wrap items-center gap-2 text-sm text-muted">
        <CountdownRing fraction={fractionLeft(left, life)} />
        <span aria-live="off">{clock(left)} left</span>
        <span class="flex-1"></span>
        <Button onclick={() => void pairing.decide(false)}>Deny</Button>
        <Button variant="primary" onclick={() => void pairing.decide(true)}>Allow</Button>
      </div>
    {:else if view.state === 'preview'}
      {@render heading(title)}
      <p class="mb-3 text-sm text-muted">Both devices have data. Everything is combined; nothing is removed.</p>
      <div class="grid grid-cols-3 gap-2" role="list" aria-label="What each device holds">
        {#each previewStats(view.there, view.here) as stat (stat.label)}
          <div role="listitem" class="rounded-sm bg-surface-2 px-2.5 py-2 text-xs text-muted">
            <b class="block text-lg font-semibold text-ink tabular-nums">{stat.n}</b>{stat.label}
          </div>
        {/each}
      </div>
      {#if view.combine.length > 0}
        <p class="mt-3 text-sm text-ink">
          Lists with the same name become one: {#each view.combine as name, i (name)}<b>{name}</b>{i < view.combine.length - 1 ? ', ' : ''}{/each}.
        </p>
      {/if}
      <p class="mt-3 flex items-center gap-1.5 text-xs text-muted"><Archive class="size-3.5" />Both databases are backed up first.</p>
      <div class="mt-4 flex justify-end gap-2">
        <Button onclick={() => void pairing.merge(false)}>Cancel</Button>
        <Button variant="primary" onclick={() => void pairing.merge(true)}>Merge and sync</Button>
      </div>
    {:else if view.state === 'confirming'}
      {@render heading(title)}
      <div class="my-3 flex items-center gap-2.5 text-muted"><CountdownRing fraction={1} still />Waiting for {view.name} to confirm the merge…</div>
      <div class="mt-4 flex justify-end"><Button onclick={() => void pairing.close()}>Cancel</Button></div>
    {:else if view.state === 'finishing'}
      {@render heading(title)}
      <div class="my-3 flex items-center gap-2.5 text-muted"><CountdownRing fraction={1} still />Pairing with {view.name}…</div>
    {:else if view.state === 'waiting'}
      {@render heading(title)}
      <div class="my-3 flex items-center gap-2.5 text-muted"><CountdownRing fraction={1} still />Waiting for {view.name} to allow…</div>
      <div class="mt-4 flex justify-end"><Button onclick={() => void pairing.close()}>Cancel</Button></div>
    {:else if view.state === 'searching'}
      {@render heading(title)}
      <div class="my-3 flex items-center gap-2.5 text-muted"><CountdownRing fraction={1} still />{byAddress ? `Reaching ${pairing.address.trim()}…` : 'Looking for the device showing the code…'}</div>
      <div class="mt-4 flex justify-end"><Button onclick={() => void pairing.close()}>Cancel</Button></div>
    {:else if byAddress}
      {@render heading(title)}
      <p class="mb-3 text-sm text-muted">For a device that isn’t on this Wi-Fi, e.g. over Tailscale.</p>
      <form
        onsubmit={(e) => {
          e.preventDefault();
          void pairing.submitAddress();
        }}
      >
        <div class="mb-1 text-xs text-muted">Other device’s address</div>
        <Input
          label="Other device’s address"
          placeholder="mac.tail1234.ts.net"
          bind:value={() => pairing.address, (v) => pairing.editAddress(v)}
          invalid={addressWrong || pairing.addressError !== null}
          onblur={() => pairing.checkAddress()}
          autocapitalize="off"
          autocorrect="off"
          spellcheck={false}
          autofocus
        />
        {#if pairing.addressError}
          <p role="alert" class="mt-1 text-sm text-danger">{pairing.addressError}</p>
        {/if}
        <div class="mt-3 mb-1 text-xs text-muted">Code</div>
        <CodeInput bind:value={() => pairing.code, (v) => pairing.editCode(v)} label="Pairing code" invalid={wrongCode && pairing.code.length > 0} />
        {#if failure}{@render banner(failure.title, failure.fix)}{/if}
        <div class="mt-4 flex items-center gap-2">
          <button type="button" class="text-sm text-hue-sync underline-offset-2 hover:underline" onclick={() => void pairing.switchTo('enter')}>
            Enter a code instead
          </button>
          <span class="flex-1"></span>
          <Button onclick={() => void pairing.close()}>Cancel</Button>
          <Button type="submit" variant="primary" disabled={pairing.code.length !== 8 || pairing.address.trim() === ''}>
            {view.state === 'failed' && !wrongCode ? 'Retry' : 'Pair'}
          </Button>
        </div>
      </form>
    {:else if typing}
      {@render heading(title)}
      <p class="mb-3 text-sm text-muted">Type the code shown on your other device.</p>
      <form
        onsubmit={(e) => {
          e.preventDefault();
          void pairing.submit();
        }}
      >
        <CodeInput bind:value={() => pairing.code, (v) => pairing.editCode(v)} label="Pairing code" invalid={wrongCode && pairing.code.length > 0} autofocus />
        {#if failure}{@render banner(failure.title, failure.fix)}{/if}
        <p class="mt-3 text-sm text-muted">
          Not on the same Wi-Fi?
          <button type="button" class="text-hue-sync underline-offset-2 hover:underline" onclick={() => void pairing.switchTo('address')}>
            Pair by address…
          </button>
        </p>
        <div class="mt-4 flex items-center gap-2">
          <button type="button" class="text-sm text-hue-sync underline-offset-2 hover:underline" onclick={() => void pairing.switchTo('show')}>
            Show a code instead
          </button>
          <span class="flex-1"></span>
          <Button onclick={() => void pairing.close()}>Cancel</Button>
          <Button type="submit" variant="primary" disabled={pairing.code.length !== 8}>
            {view.state === 'failed' && !wrongCode ? 'Retry' : 'Pair'}
          </Button>
        </div>
      </form>
    {:else}
      {@render heading(title)}
      <p class="text-sm text-muted">On your other device, open Sync › Pair a device › <b>Enter a code</b>, and type:</p>
      {#if view.state === 'showing'}
        <div class="py-2 text-center font-mono text-[2.4rem] leading-tight font-semibold tracking-[0.08em] text-ink" aria-label="Code {view.code}">{view.code}</div>
        <div class="flex items-center justify-center gap-2 text-sm text-muted">
          <CountdownRing fraction={fractionLeft(left, life)} />
          <span>{clock(left)} left</span>
          {#if view.tries_left < 3}<span>· {phrase(view.tries_left)} left on this code</span>{/if}
        </div>
        {#if !view.discoverable}
          <p class="mt-3 text-center text-sm text-muted">
            "Visible on this network" is off, so the other device cannot find this one. Turn it on in Settings › Sync.
          </p>
        {/if}
        {#if notice}{@render banner(notice.title, notice.fix)}{/if}
      {:else if view.state === 'failed' && failure}
        {@render banner(failure.title, failure.fix)}
      {/if}
      <div class="mt-4 flex items-center gap-2">
        <button type="button" class="text-sm text-hue-sync underline-offset-2 hover:underline" onclick={() => void pairing.switchTo('enter')}>
          Enter a code instead
        </button>
        <span class="flex-1"></span>
        <Button variant="ghost" onclick={() => void pairing.newCode()}>New code</Button>
        <Button onclick={() => void pairing.close()}>Cancel</Button>
      </div>
    {/if}
  </div>
</Sheet>
