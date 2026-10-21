<!--
  Sheet: a bottom sheet (the phone "More" menu, pickers). Marked `data-editor-popup` so an open
  Entry editor does not close when the sheet is used. `restoreFocus` decides where focus goes on
  close instead of back to whatever opened it. `centered` makes it a panel in the middle of the
  window on desktop (still a bottom sheet on a phone); `bare` leaves the padding and the title row
  to the caller, who lays the whole sheet out (the title is then for screen readers only), and
  `autoFocus={false}` keeps focus on the sheet itself instead of its first field. On a phone every
  sheet can be pulled down to close (see sheet-swipe.ts), rises above the on-screen keyboard, and
  keeps clear of the navigation bar at the bottom; `tall` makes it nearly full height (the Task sheet).
-->
<script lang="ts">
  import { Dialog } from 'bits-ui';
  import { type Snippet, untrack } from 'svelte';
  import X from '~icons/lucide/x';
  import { keyboard } from '../keyboard.svelte';
  import { closeOnBack } from '../router/back-button.svelte';
  import { viewport } from '../viewport.svelte';
  import IconButton from './IconButton.svelte';
  import { openSheets } from './sheet-open.svelte';
  import { sheetSwipe } from './sheet-swipe';

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
    /** Nearly full height on a phone, for a sheet that is a page of its own. */
    tall?: boolean;
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
    tall = false,
    children,
  }: Props = $props();

  let panel = $state<HTMLElement | null>(null);

  // Android Back closes the sheet instead of leaving the screen.
  closeOnBack(() => open, () => onclose());

  // The toast host moves out of a sheet's way while one is open on a phone.
  $effect(() => {
    if (!open || !viewport.isPhone) return;
    untrack(() => (openSheets.count += 1));
    return () => {
      openSheets.count -= 1;
    };
  });

  // Pull down to close, on a phone only (a centred panel on a desktop has nothing to pull).
  $effect(() => {
    if (!open || !viewport.isPhone || !panel) return;
    return sheetSwipe(panel, onclose, () =>
      document.documentElement.dataset.reduceMotion === 'true' ||
      window.matchMedia('(prefers-reduced-motion: reduce)').matches,
    );
  });

  // With the keyboard up the sheet rides on it; otherwise it keeps clear of the navigation bar.
  const lift = $derived(viewport.isPhone && keyboard.open ? keyboard.inset : 0);
  const clear = $derived(lift > 0 ? '0px' : 'env(safe-area-inset-bottom)');
  const sheetStyle = $derived(
    `padding-bottom: ${bare ? clear : `calc(0.75rem + ${clear})`};` +
      (lift > 0 ? ` bottom: ${lift}px; max-height: calc(90dvh - ${lift}px);` : ''),
  );
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
        // On a phone nothing is focused on open: a ring on the first button helps no one.
        if (autoFocus && !viewport.isPhone) return;
        e.preventDefault();
        panel?.focus();
      }}
      onCloseAutoFocus={(e) => {
        if (!restoreFocus) return;
        e.preventDefault();
        restoreFocus();
      }}
      style={sheetStyle}
      class="fixed inset-x-0 bottom-0 z-50 max-h-[90dvh] overflow-y-auto overscroll-contain rounded-t-lg bg-pop shadow-pop outline-none
        {tall ? 'phone:h-[88dvh] phone:max-h-[88dvh]' : ''}
        {bare
        ? ''
        : 'pt-3 pr-[calc(0.75rem+env(safe-area-inset-right))] pl-[calc(0.75rem+env(safe-area-inset-left))]'}
        {centered
        ? 'desktop:inset-x-auto desktop:top-1/2 desktop:bottom-auto desktop:left-1/2 desktop:max-h-[86dvh] desktop:w-[min(36rem,92vw)] desktop:-translate-x-1/2 desktop:-translate-y-1/2 desktop:rounded-lg'
        : ''}"
    >
      {#if bare}
        <div aria-hidden="true" class="pointer-events-none absolute inset-x-0 top-0 z-10 flex justify-center pt-2 desktop:hidden">
          <span class="h-1 w-9 rounded-full bg-[color-mix(in_oklch,var(--ink)_20%,transparent)]"></span>
        </div>
        <Dialog.Title class="sr-only">{title}</Dialog.Title>
      {:else}
        <div aria-hidden="true" class="mx-auto -mt-2 mb-1.5 h-1 w-9 rounded-full bg-[color-mix(in_oklch,var(--ink)_20%,transparent)] desktop:hidden"></div>
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
