<!--
  EmojiPicker: choose one emoji, or none. A curated grid of 48 plus a field to type or paste any
  emoji (picked the moment one appears in it). A popover next to `anchor` on desktop, a bottom
  Sheet on a phone. `value` is the current emoji (ringed in the grid); `onpick` gets the emoji, or
  null for "No emoji". `open` is bindable. The grid and the emoji-finding rule live in `emoji.ts`.
-->
<script lang="ts">
  import Input from './Input.svelte';
  import Popover from './Popover.svelte';
  import Sheet from './Sheet.svelte';
  import { viewport } from '../viewport.svelte';
  import { CURATED_EMOJI, firstEmoji } from './emoji';

  interface Props {
    open: boolean;
    /** The button the popover hangs off (desktop). */
    anchor: HTMLElement | null;
    value: string | null;
    onpick: (emoji: string | null) => void;
    title?: string;
    /** Where focus goes when the popover closes; the anchor by default. */
    restoreFocus?: () => void;
  }
  let {
    open = $bindable(),
    anchor,
    value,
    onpick,
    title = 'Choose an emoji',
    restoreFocus,
  }: Props = $props();

  let typed = $state('');

  $effect(() => {
    if (open) typed = '';
  });

  function pick(emoji: string | null) {
    open = false;
    onpick(emoji);
  }

  function onTyped() {
    const emoji = firstEmoji(typed);
    if (emoji) pick(emoji);
  }
</script>

{#snippet body()}
  <div class="flex flex-col gap-2.5 desktop:w-70">
    <div role="group" aria-label="Emoji" class="grid grid-cols-8 gap-0.5">
      {#each CURATED_EMOJI as emoji (emoji)}
        <button
          type="button"
          aria-label={emoji}
          aria-pressed={emoji === value}
          onclick={() => pick(emoji)}
          class="grid aspect-square cursor-pointer place-items-center rounded-sm text-lg leading-none transition-colors duration-(--dur) ease-out
            hover:bg-hover phone:text-2xl {emoji === value ? 'bg-active' : ''}"
        >
          {emoji}
        </button>
      {/each}
    </div>
    <Input
      bind:value={typed}
      label="Type or paste any emoji"
      placeholder="Type or paste any emoji"
      autocomplete="off"
      oninput={onTyped}
    />
    {#if value}
      <button
        type="button"
        onclick={() => pick(null)}
        class="cursor-pointer self-start rounded-sm px-1.5 py-1 text-xs text-muted transition-colors duration-(--dur) ease-out hover:bg-hover hover:text-ink"
      >
        No emoji
      </button>
    {/if}
  </div>
{/snippet}

{#if viewport.isPhone}
  <Sheet {open} {title} onclose={() => (open = false)} {restoreFocus} closable>{@render body()}</Sheet>
{:else}
  <Popover bind:open {anchor} label={title} {restoreFocus}>{@render body()}</Popover>
{/if}
