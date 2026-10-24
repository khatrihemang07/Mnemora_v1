<!--
  ComparePage (/sync/conflicts/‹id›): the version showing and the other one side by side (stacked on a
  phone), the words that differ tinted, "Show original" for what both devices started from, and Keep
  this · Keep other · Edit merged · Later. Edit merged is two steps: choose for each clash, then polish
  the text in the Markdown editor and Save.
-->
<script lang="ts">
  import { createQuery } from '@tanstack/svelte-query';
  import ArrowLeft from '~icons/lucide/arrow-left';
  import Check from '~icons/lucide/check';
  import GitMerge from '~icons/lucide/git-merge';
  import { type ClashChoice, commands, type ConflictChoice, type DiffSpan } from '../../lib/ipc/bindings';
  import { unwrap } from '../../lib/ipc/unwrap';
  import { readCore } from '../../lib/settings/core';
  import { settingsQuery } from '../../lib/settings/queries';
  import { closeOnBack } from '../../lib/router/back-button.svelte';
  import Button from '../../lib/ui/Button.svelte';
  import Card from '../../lib/ui/Card.svelte';
  import EmptyState from '../../lib/ui/EmptyState.svelte';
  import ErrorBanner from '../../lib/ui/ErrorBanner.svelte';
  import MarkdownEditor from '../../lib/ui/MarkdownEditor.svelte';
  import Skeleton from '../../lib/ui/Skeleton.svelte';
  import Switch from '../../lib/ui/Switch.svelte';
  import { notify, notifyError } from '../../lib/ui/toast';
  import { moduleContext } from '../../shell/module-context';
  import { router } from '../../shell/router';
  import { conflictIdOf, rowTitle, timeText, usePossessive } from './conflicts';
  import { compareFlow, forgetCompareFlow, stepBack } from './compare-flow.svelte';
  import { conflictQuery, watchConflicts } from './queries';
  import SyncFrame from './SyncFrame.svelte';

  const id = $derived(conflictIdOf(router.pathname) ?? '');
  const conflict = createQuery(() => conflictQuery(id));
  const view = $derived(conflict.data ?? null);
  const settings = createQuery(() => settingsQuery);
  const timeFormat = $derived(readCore(settings.data?.values).timeFormat);

  $effect(() => {
    let stop: (() => void) | undefined;
    let gone = false;
    void watchConflicts().then((off) => (gone ? off() : (stop = off)));
    return () => {
      gone = true;
      stop?.();
    };
  });

  // Kept per conflict outside this component: the shell remounts the page when the window crosses the
  // phone breakpoint, and the step, choices and text must survive that.
  const flow = $derived(compareFlow(id));
  const showOriginal = $derived(flow.showOriginal);
  const step = $derived(flow.step);
  const choices = $derived(flow.choices);
  const draft = $derived(flow.draft);
  let busy = $state(false);

  // Back inside Edit merged goes one step back and keeps the draft, as Back finishes any transient mode.
  closeOnBack(
    () => stepBack(step) !== null,
    () => {
      const previous = stepBack(flow.step);
      if (previous) flow.step = previous;
    },
  );

  const clashes = $derived(view?.pieces.filter((p) => p.kind === 'clash') ?? []);
  const showingName = $derived(view?.row.showing.device ?? '');
  const otherName = $derived(view?.row.other.device ?? '');

  const back = () => moduleContext.navigate('/sync/conflicts');

  async function settle(choice: ConflictChoice, message: string) {
    busy = true;
    try {
      await unwrap(commands.syncConflictResolve(id, choice));
      notify(message);
      forgetCompareFlow(id);
      back();
    } catch (e) {
      notifyError(e instanceof Error ? e.message : 'Could not settle that conflict');
    } finally {
      busy = false;
    }
  }

  async function startEdit() {
    flow.choices = clashes.map(() => 'showing');
    if (clashes.length === 0) await toPolish();
    else flow.step = 'choose';
  }

  async function toPolish() {
    flow.draft = await unwrap(commands.syncConflictDraft(id, flow.choices));
    flow.step = 'polish';
  }

  function pick(index: number, value: ClashChoice) {
    flow.choices = flow.choices.map((c, i) => (i === index ? value : c));
  }

  const numbered = $derived.by(() => {
    let next = 0;
    return (view?.pieces ?? []).map((p) => ({ p, n: p.kind === 'clash' ? next++ : -1 }));
  });
</script>

{#snippet words(spans: DiffSpan[], tint: string, removed = false)}
  <p class="font-display leading-relaxed whitespace-pre-wrap">
    {#each spans as s}{#if s.changed}<mark class="rounded-[3px] px-px text-ink {tint} {removed ? 'line-through' : ''}">{s.text}</mark>{:else}{s.text}{/if}{/each}
  </p>
{/snippet}

<!-- The Sync header and tabs stay (Conflicts is the one on show), at the width of the other Sync pages. -->
<SyncFrame tab="conflicts">
    <div class="mb-2 flex items-center gap-2">
      <Button variant="ghost" size="sm" onclick={back}>
        <ArrowLeft class="size-3.5" /> Conflicts
      </Button>
      <span class="flex-1"></span>
      {#if step === 'compare' && view}
        <label class="flex items-center gap-2 text-sm text-muted">
          <Switch checked={showOriginal} label="Show original" onchange={(v) => (flow.showOriginal = v)} />
          Show original
        </label>
      {/if}
    </div>

    {#if conflict.isError}
      <ErrorBanner error={conflict.error} onretry={() => conflict.refetch()} />
    {:else if conflict.isPending}
      <Skeleton class="h-48 w-full" />
    {:else if view === null}
      <Card>
        <EmptyState
          icon={GitMerge}
          hue="--hue-sync"
          title="That conflict is gone"
          description="It was settled on another device."
        >
          {#snippet action()}<Button size="sm" onclick={back}>Back to Conflicts</Button>{/snippet}
        </EmptyState>
      </Card>
    {:else}
      <h1 class="font-display text-xl font-semibold">{step === 'compare' ? rowTitle(view.row.subject, timeFormat) : 'Edit merged'}</h1>
      {#if step === 'compare'}
        <p class="mb-3 text-sm text-muted">
          Both devices rewrote the same sentence. {showingName}’s version shows in the {view.row.module === 'journal'
            ? 'Journal'
            : 'Task'} for now.
        </p>
        {#if !view.open}
          <p class="mb-3 rounded-md bg-hue-sync-soft px-3 py-2 text-sm" data-resolved>This conflict is settled.</p>
        {/if}
        <div class="grid gap-2.5 {showOriginal ? 'desktop:grid-cols-3' : 'desktop:grid-cols-2'}">
          {#if showOriginal}
            <section class="overflow-hidden rounded-md border border-line bg-surface" data-side="original">
              <header class="border-b border-line px-3 py-2 text-xs text-muted"><b class="text-ink">Original</b> before both</header>
              <div class="px-3.5 py-2.5"><p class="font-display leading-relaxed whitespace-pre-wrap">{view.original}</p></div>
            </section>
          {/if}
          <section class="overflow-hidden rounded-md border border-hue-sync bg-surface" data-side="showing">
            <header class="border-b border-line px-3 py-2 text-xs text-muted">
              <b class="text-ink">Showing</b> <span class="wrap-anywhere">{showingName}</span> · {timeText(view.row.showing.at, timeFormat)}
            </header>
            <div class="px-3.5 py-2.5">{@render words(view.showing, 'bg-diff-add')}</div>
          </section>
          <section class="overflow-hidden rounded-md border border-line bg-surface" data-side="other">
            <header class="border-b border-line px-3 py-2 text-xs text-muted">
              <b class="text-ink">Other</b> <span class="wrap-anywhere">{otherName}</span> · {timeText(view.row.other.at, timeFormat)}
            </header>
            <div class="px-3.5 py-2.5">{@render words(view.other, 'bg-diff-del', true)}</div>
          </section>
        </div>
        {#if view.open}
          <div class="mt-3 flex flex-wrap gap-1.5">
            <Button variant="primary" class="max-w-full h-auto! min-h-7 py-1 whitespace-normal! text-left wrap-anywhere" disabled={busy} onclick={() => settle({ kind: 'keep_this' }, 'Kept this version')}>
              Keep {showingName}’s
            </Button>
            <Button class="max-w-full h-auto! min-h-7 py-1 whitespace-normal! text-left wrap-anywhere" disabled={busy} onclick={() => settle({ kind: 'keep_other' }, `Kept ${otherName}’s version`)}>
              Keep {otherName}’s
            </Button>
            <Button disabled={busy} onclick={startEdit}>Edit merged…</Button>
            <Button variant="ghost" onclick={back}>Later</Button>
          </div>
        {/if}
      {:else if step === 'choose'}
        <p class="mb-3 text-xs text-faint"><b class="text-hue-sync">1 Choose for each clash</b> › 2 Polish</p>
        <Card>
          <div class="font-display leading-relaxed whitespace-pre-wrap">
            {#each numbered as { p, n }}
              {#if p.kind === 'agreed'}
                {p.text}
              {:else}
                <!-- whitespace-normal: the text around is pre-wrap, which would draw the template's line breaks between the rows as blank bands. -->
                <span class="my-2 block rounded-sm border border-line font-sans text-sm whitespace-normal" data-clash={n}>
                  <span class="block bg-diff-add border-l-3 px-2.5 py-1.5 {choices[n] !== 'other' ? 'border-l-hue-sync' : 'border-l-transparent'}">
                    <b class="wrap-anywhere">{showingName}</b> {p.showing}
                  </span>
                  <span class="block bg-diff-del border-l-3 px-2.5 py-1.5 {choices[n] !== 'showing' ? 'border-l-hue-sync' : 'border-l-transparent'}">
                    <b class="wrap-anywhere">{otherName}</b> {p.other}
                  </span>
                  <!-- Buttons, not a SegmentedControl: the device names are long, so they wrap (and stack on a phone), and the chosen one carries a check as well as the tint. -->
                  <span
                    class="flex flex-wrap gap-1 px-2.5 py-2 phone:flex-col"
                    role="radiogroup"
                    aria-label="Which words for clash {n + 1}"
                  >
                    {#each [['showing', usePossessive(showingName)], ['other', usePossessive(otherName)], ['both', 'Both']] as [value, text] (value)}
                      {@const chosen = (choices[n] ?? 'showing') === value}
                      <Button
                        size="sm"
                        role="radio"
                        aria-checked={chosen}
                        data-chosen={chosen}
                        class="max-w-full h-auto! min-h-7 py-1 whitespace-normal! text-left wrap-anywhere phone:justify-start {chosen ? 'border-hue-sync! bg-hue-sync-soft!' : ''}"
                        onclick={() => pick(n, value as ClashChoice)}
                      >
                        {#if chosen}<Check class="size-3.5 shrink-0" />{/if}
                        <span>{text}</span>
                      </Button>
                    {/each}
                  </span>
                </span>
              {/if}
            {/each}
          </div>
        </Card>
        <div class="mt-3 flex flex-wrap gap-1.5">
          <Button variant="primary" onclick={toPolish}><Check class="size-3.5" /> Next: polish</Button>
          <Button variant="ghost" onclick={() => (flow.step = 'compare')}>Cancel</Button>
        </div>
      {:else}
        <p class="mb-3 text-xs text-faint">1 Choose for each clash › <b class="text-hue-sync">2 Polish</b></p>
        <Card class="min-h-40">
          <MarkdownEditor label="Merged text" value={draft} onchange={(t) => (flow.draft = t)} bare />
        </Card>
        <div class="mt-3 flex flex-wrap gap-1.5">
          <Button
            variant="primary"
            disabled={busy || draft.trim() === ''}
            onclick={() => settle({ kind: 'merged', text: draft }, 'Saved the merged text')}
          >
            Save
          </Button>
          <Button variant="ghost" onclick={() => (flow.step = 'compare')}>Cancel</Button>
        </div>
      {/if}
    {/if}
</SyncFrame>
