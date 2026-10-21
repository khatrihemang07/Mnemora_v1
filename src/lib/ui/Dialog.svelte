<!-- Dialog: small modal with a title, body and an actions row. -->
<script lang="ts">
  import { Dialog } from 'bits-ui';
  import type { Snippet } from 'svelte';
  import { closeOnBack } from '../router/back-button.svelte';

  interface Props {
    open: boolean;
    title: string;
    description?: string;
    onclose: () => void;
    /** Content between the description and the actions: choices, a field. */
    children?: Snippet;
    actions: Snippet;
  }
  let { open, title, description, onclose, children, actions }: Props = $props();

  closeOnBack(() => open, () => onclose());
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
      {#if children}<div class="mt-4">{@render children()}</div>{/if}
      <div class="mt-5 flex justify-end gap-2">{@render actions()}</div>
    </Dialog.Content>
  </Dialog.Portal>
</Dialog.Root>
