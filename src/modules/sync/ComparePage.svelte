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
  import Button from '../../lib/ui/Button.svelte';
  import Card from '../../lib/ui/Card.svelte';
  import EmptyState from '../../lib/ui/EmptyState.svelte';
  import ErrorBanner from '../../lib/ui/ErrorBanner.svelte';
  import MarkdownEditor from '../../lib/ui/MarkdownEditor.svelte';
  import SegmentedControl from '../../lib/ui/SegmentedControl.svelte';
  import Skeleton from '../../lib/ui/Skeleton.svelte';
  import Switch from '../../lib/ui/Switch.svelte';
  import { notify, notifyError } from '../../lib/ui/toast';
  import { moduleContext } from '../../shell/module-context';
  import { router } from '../../shell/router';
  import { clockText, conflictIdOf, rowTitle, usePossessive } from './conflicts';
  import { conflictQuery, watchConflicts } from './queries';

  const id = $derived(conflictIdOf(router.pathname) ?? '');
  const conflict = createQuery(() => conflictQuery(id));
  const view = $derived(conflict.data ?? null);

  $effect(() => {
    let stop: (() => void) | undefined;
    let gone = false;
    void watchConflicts().then((off) => (gone ? off() : (stop = off)));
    return () => {
      gone = true;
      stop?.();
    };
  });

  let showOriginal = $state(false);
  let step = $state<'compare' | 'choose' | 'polish'>('compare');
  let choices = $state<ClashChoice[]>([]);
  let draft = $state('');
  let busy = $state(false);

  const clashes = $derived(view?.pieces.filter((p) => p.kind === 'clash') ?? []);
  const showingName = $derived(view?.row.showing.device ?? '');
  const otherName = $derived(view?.row.other.device ?? '');

  const back = () => moduleContext.navigate('/sync/conflicts');

  async function settle(choice: ConflictChoice, message: string) {
    busy = true;
    try {
      await unwrap(commands.syncConflictResolve(id, choice));
      notify(message);
      back();
    } catch (e) {
      notifyError(e instanceof Error ? e.message : 'Could not settle that conflict');
    } finally {
      busy = false;
    }
  }

  async function startEdit() {
    choices = clashes.map(() => 'showing');
    if (clashes.length === 0) await toPolish();
    else step = 'choose';
  }

  async function toPolish() {
    draft = await unwrap(commands.syncConflictDraft(id, choices));
    step = 'polish';
  }

  function pick(index: number, value: ClashChoice) {
    choices = choices.map((c, i) => (i === index ? value : c));
  }

  const numbered = $derived.by(() => {
    let next = 0;
    return (view?.pieces ?? []).map((p) => ({ p, n: p.kind === 'clash' ? next++ : -1 }));
  });
</script>

{#snippet words(spans: DiffSpan[], tint: string)}
  <p class="font-display leading-relaxed whitespace-pre-wrap">
    {#each spans as s}{#if s.changed}<mark class="rounded-[3px] px-px text-ink {tint}">{s.text}</mark>{:else}{s.text}{/if}{/each}
  </p>
{/snippet}

<div data-sync class="selectable h-full overflow-y-auto p-(--pad)">
  <div class="mx-auto max-w-4xl">
    <div class="mb-2 flex items-center gap-2">
      <Button variant="ghost" size="sm" onclick={back}>
        <ArrowLeft class="size-3.5" /> Conflicts
      </Button>
      <span class="flex-1"></span>
      {#if step === 'compare' && view}
        <label class="flex items-center gap-2 text-sm text-muted">
          <Switch checked={showOriginal} label="Show original" onchange={(v) => (showOriginal = v)} />
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
      <h1 class="font-display text-xl font-semibold">{step === 'compare' ? rowTitle(view.row.subject) : 'Edit merged'}</h1>
      {#if step === 'compare'}
        <p class="mb-3 text-sm text-muted">
          Both devices rewrote the same words. {showingName}’s version shows in the {view.row.module === 'journal'
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
              <b class="text-ink">Showing</b> {showingName} · {clockText(view.row.showing.at)}
            </header>
            <div class="px-3.5 py-2.5">{@render words(view.showing, 'bg-diff-add')}</div>
          </section>
          <section class="overflow-hidden rounded-md border border-line bg-surface" data-side="other">
            <header class="border-b border-line px-3 py-2 text-xs text-muted">
              <b class="text-ink">Other</b> {otherName} · {clockText(view.row.other.at)}
            </header>
            <div class="px-3.5 py-2.5">{@render words(view.other, 'bg-diff-del')}</div>
          </section>
        </div>
        {#if view.open}
          <div class="mt-3 flex flex-wrap gap-1.5">
            <Button variant="primary" disabled={busy} onclick={() => settle({ kind: 'keep_this' }, 'Kept this version')}>
              Keep {showingName}’s
            </Button>
            <Button disabled={busy} onclick={() => settle({ kind: 'keep_other' }, `Kept ${otherName}’s version`)}>
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
                <span class="my-2 block rounded-sm border border-line font-sans text-sm" data-clash={n}>
                  <span class="block bg-diff-add px-2.5 py-1.5 {choices[n] !== 'other' ? 'shadow-[inset_3px_0_0_var(--hue-sync)]' : ''}">
                    <b>{showingName}</b> {p.showing}
                  </span>
                  <span class="block bg-diff-del px-2.5 py-1.5 {choices[n] !== 'showing' ? 'shadow-[inset_3px_0_0_var(--hue-sync)]' : ''}">
                    <b>{otherName}</b> {p.other}
                  </span>
                  <span class="block px-2.5 py-2">
                    <SegmentedControl
                      label="Which words for clash {n + 1}"
                      value={choices[n] ?? 'showing'}
                      options={[
                        { value: 'showing', label: usePossessive(showingName) },
                        { value: 'other', label: usePossessive(otherName) },
                        { value: 'both', label: 'Both' },
                      ]}
                      onchange={(v) => pick(n, v)}
                    />
                  </span>
                </span>
              {/if}
            {/each}
          </div>
        </Card>
        <div class="mt-3 flex flex-wrap gap-1.5">
          <Button variant="primary" onclick={toPolish}><Check class="size-3.5" /> Next: polish</Button>
          <Button variant="ghost" onclick={() => (step = 'compare')}>Cancel</Button>
        </div>
      {:else}
        <p class="mb-3 text-xs text-faint">1 Choose for each clash › <b class="text-hue-sync">2 Polish</b></p>
        <Card class="min-h-40">
          <MarkdownEditor label="Merged text" value={draft} onchange={(t) => (draft = t)} bare />
        </Card>
        <div class="mt-3 flex flex-wrap gap-1.5">
          <Button
            variant="primary"
            disabled={busy || draft.trim() === ''}
            onclick={() => settle({ kind: 'merged', text: draft }, 'Saved the merged text')}
          >
            Save
          </Button>
          <Button variant="ghost" onclick={() => (step = 'compare')}>Cancel</Button>
        </div>
      {/if}
    {/if}
  </div>
</div>
