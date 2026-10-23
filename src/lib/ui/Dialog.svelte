<!--
  Dialog: small modal with a title, body and an actions row. `overSheet` lifts it above a Sheet
  (z-50), for a Dialog opened from inside one: it then dims the whole Sheet, not only the page.
  `locked` keeps it open while work is under way: Escape, a click outside and Back do nothing.
-->
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
    overSheet?: boolean;
    locked?: boolean;
  }
  let { open, title, description, onclose, children, actions, overSheet = false, locked = false }: Props = $props();

  closeOnBack(() => open, () => !locked && onclose());
</script>

<Dialog.Root {open} onOpenChange={(o) => !o && !locked && onclose()}>
  <Dialog.Portal>
    <Dialog.Overlay class="fixed inset-0 {overSheet ? 'z-60' : 'z-40'} bg-scrim" />
    <Dialog.Content
      escapeKeydownBehavior={locked ? 'ignore' : 'close'}
      interactOutsideBehavior={locked ? 'ignore' : 'close'}
      class="fixed top-1/2 left-1/2 {overSheet ? 'z-70' : 'z-50'} w-[min(92vw,26rem)] -translate-x-1/2 -translate-y-1/2 rounded-md bg-pop p-5 shadow-pop"
    >
      <Dialog.Title class="text-base font-semibold text-ink">{title}</Dialog.Title>
      {#if description}
        <Dialog.Description class="mt-1.5 text-muted">{description}</Dialog.Description>
      {/if}
      {#if children}<div class="mt-4">{@render children()}</div>{/if}
      <div class="mt-5 flex flex-wrap justify-end gap-2">{@render actions()}</div>
    </Dialog.Content>
  </Dialog.Portal>
</Dialog.Root>
