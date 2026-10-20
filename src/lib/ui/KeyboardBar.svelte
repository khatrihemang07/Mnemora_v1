<!--
  KeyboardBar: the phone's formatting row, the last row of the page, so it sits right above the
  on-screen keyboard (the app shell keeps the page above it). Shown only while an editor has focus
  and the keyboard is up. Bold, italic, heading, bullet, checklist, quote and link run the same
  commands as the desktop shortcuts. Buttons never take focus, so the editor and keyboard stay up.
-->
<script lang="ts">
  import type { Component } from 'svelte';
  import Bold from '~icons/lucide/bold';
  import Heading from '~icons/lucide/heading';
  import Italic from '~icons/lucide/italic';
  import Link from '~icons/lucide/link';
  import List from '~icons/lucide/list';
  import ListChecks from '~icons/lucide/list-checks';
  import Quote from '~icons/lucide/quote';
  import type { EditorCommand } from './markdown/shortcuts';

  interface Props {
    visible: boolean;
    onaction: (command: EditorCommand) => void;
  }
  let { visible, onaction }: Props = $props();

  const BUTTONS: { label: string; icon: Component<{ class?: string }>; command: EditorCommand }[] = [
    { label: 'Bold', icon: Bold, command: { type: 'inline', mark: '**' } },
    { label: 'Italic', icon: Italic, command: { type: 'inline', mark: '*' } },
    { label: 'Heading', icon: Heading, command: { type: 'block', kind: 'h2' } },
    { label: 'Bulleted list', icon: List, command: { type: 'block', kind: 'bullet' } },
    { label: 'Checklist', icon: ListChecks, command: { type: 'block', kind: 'task' } },
    { label: 'Quote', icon: Quote, command: { type: 'block', kind: 'quote' } },
    { label: 'Link', icon: Link, command: { type: 'link' } },
  ];
</script>

{#if visible}
  <div
    role="toolbar"
    aria-label="Formatting"
    class="flex shrink-0 gap-0.5 border-t border-line bg-surface-2 px-1.5 py-1"
  >
    {#each BUTTONS as b (b.label)}
      <button
        type="button"
        aria-label={b.label}
        class="grid h-10 flex-1 place-items-center rounded-sm text-ink transition-[background-color,transform] duration-(--dur) ease-out active:scale-[0.96] active:bg-active"
        onpointerdown={(e) => e.preventDefault()}
        onclick={() => onaction(b.command)}
      >
        <b.icon class="size-[1.15rem]" />
      </button>
    {/each}
  </div>
{/if}
