<!--
  Sheet: a bottom sheet (the phone "More" menu, pickers). Marked `data-editor-popup` so an open
  Entry editor does not close when the sheet is used.
-->
<script lang="ts">
  import { Dialog } from 'bits-ui';
  import type { Snippet } from 'svelte';

  interface Props {
    open: boolean;
    title: string;
    onclose: () => void;
    children: Snippet;
  }
  let { open, title, onclose, children }: Props = $props();
</script>

<Dialog.Root {open} onOpenChange={(o) => !o && onclose()}>
  <Dialog.Portal>
    <Dialog.Overlay data-editor-popup class="fixed inset-0 z-40 bg-scrim" />
    <Dialog.Content
      data-editor-popup
      class="fixed inset-x-0 bottom-0 z-50 rounded-t-lg bg-pop p-3 pb-[calc(0.75rem+env(safe-area-inset-bottom))] shadow-pop"
    >
      <Dialog.Title class="px-2 pt-1 pb-2 text-xs font-medium text-faint">{title}</Dialog.Title>
      {@render children()}
    </Dialog.Content>
  </Dialog.Portal>
</Dialog.Root>
