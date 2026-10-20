<!-- Dialog: small modal with a title, body and an actions row. -->
<script lang="ts">
  import { Dialog } from 'bits-ui';
  import type { Snippet } from 'svelte';

  interface Props {
    open: boolean;
    title: string;
    description?: string;
    onclose: () => void;
    actions: Snippet;
  }
  let { open, title, description, onclose, actions }: Props = $props();
</script>

<Dialog.Root {open} onOpenChange={(o) => !o && onclose()}>
  <Dialog.Portal>
    <Dialog.Overlay class="fixed inset-0 z-40 bg-scrim" />
    <Dialog.Content
      class="fixed top-1/2 left-1/2 z-50 w-[min(92vw,26rem)] -translate-x-1/2 -translate-y-1/2 rounded-md bg-pop p-5 shadow-pop"
    >
      <Dialog.Title class="text-base font-semibold text-ink">{title}</Dialog.Title>
      {#if description}
        <Dialog.Description class="mt-1.5 text-muted">{description}</Dialog.Description>
      {/if}
      <div class="mt-5 flex justify-end gap-2">{@render actions()}</div>
    </Dialog.Content>
  </Dialog.Portal>
</Dialog.Root>
