<!--
  Sheet: a bottom sheet (the phone "More" menu, pickers). Marked `data-editor-popup` so an open
  Entry editor does not close when the sheet is used. `restoreFocus` decides where focus goes on
  close instead of back to whatever opened it. `centered` makes it a panel in the middle of the
  window on desktop (still a bottom sheet on a phone); `bare` leaves the padding and the title row
  to the caller, who lays the whole sheet out (the title is then for screen readers only), and
  `autoFocus={false}` keeps focus on the sheet itself instead of its first field.
-->
<script lang="ts">
  import { Dialog } from 'bits-ui';
  import type { Snippet } from 'svelte';
  import X from '~icons/lucide/x';
  import { closeOnBack } from '../router/back-button.svelte';
  import IconButton from './IconButton.svelte';

  interface Props {
    open: boolean;
    title: string;
    onclose: () => void;
    restoreFocus?: () => void;
    /** Adds a Close button in the title row, for sheets with no other obvious way out. */
    closable?: boolean;
    /** Returns true when Esc belongs to something inside (a floating panel), so the sheet stays open. */
    guardEscape?: () => boolean;
    centered?: boolean;
    bare?: boolean;
    autoFocus?: boolean;
    children: Snippet;
  }
  let {
    open,
    title,
    onclose,
    restoreFocus,
    closable = false,
    guardEscape,
    centered = false,
    bare = false,
    autoFocus = true,
    children,
  }: Props = $props();

  let panel = $state<HTMLElement | null>(null);

  // Android Back closes the sheet instead of leaving the screen.
  closeOnBack(() => open, () => onclose());
</script>

<Dialog.Root {open} onOpenChange={(o) => !o && onclose()}>
  <Dialog.Portal>
    <Dialog.Overlay data-editor-popup class="fixed inset-0 z-40 bg-scrim" />
    <Dialog.Content
      bind:ref={panel}
      data-editor-popup
      data-bare={autoFocus ? undefined : ''}
      onEscapeKeydown={(e) => guardEscape?.() && e.preventDefault()}
      onInteractOutside={(e) => {
        // A toast's Undo is outside the sheet but never a reason to close it.
        if (e.target instanceof Element && e.target.closest('[data-sonner-toast]')) e.preventDefault();
      }}
      onOpenAutoFocus={(e) => {
        if (autoFocus) return;
        e.preventDefault();
        panel?.focus();
      }}
      onCloseAutoFocus={(e) => {
        if (!restoreFocus) return;
        e.preventDefault();
        restoreFocus();
      }}
      class="fixed inset-x-0 bottom-0 z-50 max-h-[90dvh] overflow-y-auto rounded-t-lg bg-pop shadow-pop outline-none
        {bare
        ? ''
        : 'pt-3 pr-[calc(0.75rem+env(safe-area-inset-right))] pb-[calc(0.75rem+env(safe-area-inset-bottom))] pl-[calc(0.75rem+env(safe-area-inset-left))]'}
        {centered
        ? 'desktop:inset-x-auto desktop:top-1/2 desktop:bottom-auto desktop:left-1/2 desktop:max-h-[86dvh] desktop:w-[min(36rem,92vw)] desktop:-translate-x-1/2 desktop:-translate-y-1/2 desktop:rounded-lg'
        : ''}"
    >
      {#if bare}
        <Dialog.Title class="sr-only">{title}</Dialog.Title>
      {:else}
        <div class="flex items-center justify-between gap-2 pb-2">
          <Dialog.Title class="px-2 pt-1 text-xs font-medium text-faint">{title}</Dialog.Title>
          {#if closable}
            <IconButton icon={X} label="Close" class="phone:size-11" onclick={onclose} />
          {/if}
        </div>
      {/if}
      {@render children()}
    </Dialog.Content>
  </Dialog.Portal>
</Dialog.Root>
