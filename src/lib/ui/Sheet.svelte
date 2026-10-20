<!--
  Sheet: a bottom sheet (the phone "More" menu, pickers). Marked `data-editor-popup` so an open
  Entry editor does not close when the sheet is used. `restoreFocus` decides where focus goes on
  close instead of back to whatever opened it.
-->
<script lang="ts">
  import { Dialog } from 'bits-ui';
  import type { Snippet } from 'svelte';
  import { closeOnBack } from '../router/back-button.svelte';

  interface Props {
    open: boolean;
    title: string;
    onclose: () => void;
    restoreFocus?: () => void;
    children: Snippet;
  }
  let { open, title, onclose, restoreFocus, children }: Props = $props();

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
      class="fixed inset-x-0 bottom-0 z-50 rounded-t-lg bg-pop p-3 pb-[calc(0.75rem+env(safe-area-inset-bottom))] shadow-pop"
    >
      <Dialog.Title class="px-2 pt-1 pb-2 text-xs font-medium text-faint">{title}</Dialog.Title>
      {@render children()}
    </Dialog.Content>
  </Dialog.Portal>
</Dialog.Root>
