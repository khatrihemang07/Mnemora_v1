<!--
  MarkdownEditor: live Markdown writing on CodeMirror 6, loaded on first use so it stays out of the
  main bundle. Symbols stay visible but faded; the text is typeset like MarkdownView. Includes
  shortcuts, list keys, auto-pairs, the slash menu, rich-text paste and ⌘/Ctrl-click links.
  `value` is the text and `onchange` reports every edit; saving is the caller's job. `onfinish` fires
  on ⌘/Ctrl+Enter and `onescape` (or `onfinish` when absent) on Esc. `checklist` draws `- [ ]` items as clickable square boxes (Todo's notes); the Journal keeps faded text. `bare` drops the editor's focus ring for use inside a frame that shows
  focus itself. Call `focus()` and `run()` (the phone KeyboardBar) through `bind:this`.
-->
<script lang="ts">
  import { onMount, tick } from 'svelte';
  import { keyboard } from '../keyboard.svelte';
  import { openExternal } from '../open-external';
  import { formatDate } from '../time/format';
  import type { EditorController, SlashState } from './markdown/editor-core';
  import type { EditorCommand } from './markdown/shortcuts';
  import type { SlashValues } from './markdown/slash';

  interface Props {
    value: string;
    onchange: (text: string) => void;
    onfinish?: () => void;
    /** Esc; falls back to `onfinish`. */
    onescape?: () => void;
    onfocuschange?: (focused: boolean) => void;
    /** The accessible name. */
    label: string;
    placeholder?: string;
    /** The body font: the display serif (default) or the UI sans. */
    font?: 'serif' | 'sans';
    spellcheck?: boolean;
    bare?: boolean;
    /** Checklist items are clickable square boxes instead of faded `- [ ]` text. */
    checklist?: boolean;
    /** Date and time the slash menu inserts, in the app's formats. */
    slashValues?: () => SlashValues;
    /** A CSS length: the editor scrolls inside it instead of growing without end. */
    maxHeight?: string;
    class?: string;
  }
  let {
    value,
    onchange,
    onfinish,
    onescape,
    onfocuschange,
    label,
    placeholder = 'Write…',
    font = 'serif',
    spellcheck = true,
    bare = false,
    checklist = false,
    slashValues = defaultSlashValues,
    maxHeight,
    class: cls = '',
  }: Props = $props();

  function defaultSlashValues(): SlashValues {
    const now = new Date();
    return {
      date: formatDate(now, 'long'),
      time: now.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' }),
    };
  }

  const listId = `md-slash-${Math.random().toString(36).slice(2, 8)}`;
  let host = $state<HTMLDivElement | null>(null);
  let ctrl = $state.raw<EditorController | null>(null);
  let slash = $state.raw<SlashState | null>(null);
  let wantsFocus = false;
  let hasFocus = $state(false);

  /** Focuses the editor (once it has loaded, if it has not yet). */
  export function focus() {
    if (ctrl) ctrl.focus();
    else wantsFocus = true;
  }

  /** Takes focus away (the Journal composer's Esc). */
  export function blur() {
    ctrl?.blur();
  }

  /** Runs a formatting command, as the phone KeyboardBar does. */
  export function run(command: EditorCommand): boolean {
    return ctrl?.run(command) ?? false;
  }

  onMount(() => {
    let gone = false;
    let made: EditorController | null = null;
    void import('./markdown/editor-core').then(({ createEditor }) => {
      if (gone || !host) return;
      made = createEditor({
        parent: host,
        doc: value,
        label,
        placeholder,
        spellcheck,
        bare,
        checklist,
        onchange: (text) => onchange(text),
        onfinish: () => onfinish?.(),
        onescape: () => (onescape ?? onfinish)?.(),
        onfocuschange: (f) => {
          if (f) {
            hasFocus = true;
            onfocuschange?.(true);
            return;
          }
          // Removing a focused editor blurs it while Svelte is destroying the page around it,
          // where writing state is an error (leaving the Journal from ⌘K with the composer
          // focused). A blur is told one microtask later, which is after any teardown.
          queueMicrotask(() => {
            hasFocus = false;
            onfocuschange?.(false);
          });
        },
        onslash: (s) => {
          if (!gone) slash = s;
        },
        slashValues: () => slashValues(),
        onopenlink: (href) => void openExternal(href),
      });
      ctrl = made;
      if (wantsFocus) made.focus();
    });
    return () => {
      gone = true;
      made?.destroy();
    };
  });

  // The text can change from outside (the composer clearing after Finish).
  $effect(() => {
    ctrl?.setDoc(value);
  });

  // The keyboard shrinks the page under a focused editor: once the layout has settled, scroll the
  // editor into view so the line being typed is not left under the composer or the keyboard.
  $effect(() => {
    // Re-runs as the keyboard's inset settles.
    if (!keyboard.open || !hasFocus || keyboard.inset < 0) return;
    const frame = requestAnimationFrame(() => host?.scrollIntoView({ block: 'nearest' }));
    return () => cancelAnimationFrame(frame);
  });

  // Spellcheck follows the Device setting while the editor is open.
  $effect(() => {
    ctrl?.setSpellcheck(spellcheck);
  });

  // Keep the active menu row visible and tell assistive tech about it.
  $effect(() => {
    const s = slash;
    ctrl?.setPopup(listId, s ? `${listId}-${s.items[s.index]?.id}` : null);
    if (s) {
      void tick().then(() =>
        document.getElementById(`${listId}-${s.items[s.index]?.id}`)?.scrollIntoView({ block: 'nearest' }),
      );
    }
  });

  const MENU_WIDTH_PX = 248;
  const MENU_HEIGHT_PX = 288;
  const MENU_GAP_PX = 6;
  const menuStyle = $derived.by(() => {
    if (!slash) return '';
    const left = Math.max(8, Math.min(slash.rect.left, window.innerWidth - MENU_WIDTH_PX - 8));
    const below = window.innerHeight - slash.rect.bottom;
    return below >= MENU_HEIGHT_PX || below >= slash.rect.top
      ? `left:${left}px;top:${slash.rect.bottom + MENU_GAP_PX}px`
      : `left:${left}px;bottom:${window.innerHeight - slash.rect.top + MENU_GAP_PX}px`;
  });
  const preview = $derived.by(() => {
    if (!slash) return null;
    const v = slashValues();
    return { date: v.date, time: v.time };
  });
</script>

<div class="md min-w-0 {cls}" data-font={font} style:--md-max-height={maxHeight}>
  <div bind:this={host} class="scroll-my-16"></div>
  {#if !ctrl}
    <!-- Until CodeMirror has loaded: the same box, so nothing jumps when it arrives. -->
    <div class="md-p {value ? '' : 'text-faint'}">{value || placeholder}</div>
  {/if}
</div>

{#if slash}
  <ul
    id={listId}
    role="listbox"
    aria-label="Insert"
    data-editor-popup
    class="fixed z-[60] max-h-72 w-[15.5rem] overflow-y-auto rounded-md border border-line bg-pop p-1 shadow-pop"
    style={menuStyle}
  >
    {#each slash.items as item, i (item.id)}
      <!-- svelte-ignore a11y_click_events_have_key_events -- the editor keeps focus and handles the keys -->
      <li
        id="{listId}-{item.id}"
        role="option"
        aria-selected={i === slash.index}
        class="flex cursor-pointer items-baseline justify-between gap-3 rounded-sm px-2.5 py-1.5 text-sm text-ink aria-selected:bg-hover"
        onmousedown={(e) => e.preventDefault()}
        onclick={() => ctrl?.pickSlash(item.id)}
      >
        <span>{item.label}</span>
        {#if item.id === 'date'}
          <span class="truncate text-xs text-faint">{preview?.date}</span>
        {:else if item.id === 'time'}
          <span class="text-xs text-faint tabular-nums">{preview?.time}</span>
        {/if}
      </li>
    {/each}
  </ul>
{/if}
