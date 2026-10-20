<!--
  Sheet: a bottom sheet (the phone "More" menu, pickers). Marked `data-editor-popup` so an open
  Entry editor does not close when the sheet is used. `restoreFocus` decides where focus goes on
  close instead of back to whatever opened it.
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
    children: Snippet;
  }
  let { open, title, onclose, restoreFocus, closable = false, children }: Props = $props();

  // Android Back closes the sheet instead of leaving the screen.
  closeOnBack(() => open, () => onclose());
</script>

<Dialog.Root {open} onOpenChange={(o) => !o && onclose()}>
  <Dialog.Portal>
    <Dialog.Overlay data-editor-popup class="fixed inset-0 z-40 bg-scrim" />
    <Dialog.Content
      data-editor-popup
      onCloseAutoFocus={(e) => {
        if (!restoreFocus) return;
        e.preventDefault();
        restoreFocus();
      }}
      class="fixed inset-x-0 bottom-0 z-50 max-h-[90dvh] overflow-y-auto rounded-t-lg bg-pop pt-3 pr-[calc(0.75rem+env(safe-area-inset-right))] pb-[calc(0.75rem+env(safe-area-inset-bottom))] pl-[calc(0.75rem+env(safe-area-inset-left))] shadow-pop"
    >
      <div class="flex items-center justify-between gap-2 pb-2">
        <Dialog.Title class="px-2 pt-1 text-xs font-medium text-faint">{title}</Dialog.Title>
        {#if closable}
          <IconButton icon={X} label="Close" class="phone:size-11" onclick={onclose} />
        {/if}
      </div>
      {@render children()}
    </Dialog.Content>
  </Dialog.Portal>
</Dialog.Root>
