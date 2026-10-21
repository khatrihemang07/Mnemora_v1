<!--
  Popover: a floating panel hung off an element, for small pickers (the date picker, the calendar).
  Controlled: the caller owns `open` and passes the `anchor` element. Bits UI gives Esc, outside-click
  and focus handling; focus returns to the anchor on dismissal, or wherever `restoreFocus` says. Clicking the anchor itself never
  counts as "outside", so the button that opens it can also close it. `focus` is an optional CSS
  selector for what to focus on open (default: the first focusable). Marked `data-editor-popup` so
  an open Entry editor does not treat clicks inside as "outside" and close. `flush` gives the Menu's tight
  padding (a list of rows); `autofocus={false}` leaves focus where it was on open (a
  panel opened by hover).
-->
<script lang="ts">
  import { Popover } from 'bits-ui';
  import type { Snippet } from 'svelte';
  import { closeOnBack } from '../router/back-button.svelte';

  interface Props {
    open: boolean;
    anchor: HTMLElement | null;
    /** Names the panel for screen readers. */
    label: string;
    side?: 'top' | 'right' | 'bottom';
    align?: 'start' | 'center' | 'end';
    focus?: string;
    /** Takes over focus on dismissal (e.g. into an editor) instead of the anchor. */
    restoreFocus?: () => void;
    flush?: boolean;
    autofocus?: boolean;
    children: Snippet;
  }
  let {
    open = $bindable(),
    anchor,
    label,
    side = 'bottom',
    align = 'start',
    focus,
    restoreFocus,
    flush = false,
    autofocus = true,
    children,
  }: Props = $props();

  closeOnBack(() => open, () => (open = false));

  let content = $state<HTMLElement | null>(null);

  function onOpenAutoFocus(e: Event) {
    if (!autofocus) {
      e.preventDefault();
      return;
    }
    if (!focus) return;
    const target = content?.querySelector<HTMLElement>(focus);
    if (!target) return;
    e.preventDefault();
    target.focus();
  }

  function onCloseAutoFocus(e: Event) {
    e.preventDefault();
    if (restoreFocus) restoreFocus();
    else anchor?.focus();
  }
</script>

<Popover.Root bind:open>
  <Popover.Portal>
    <Popover.Content
      bind:ref={content}
      customAnchor={anchor}
      {side}
      {align}
      sideOffset={6}
      collisionPadding={8}
      aria-label={label}
      data-editor-popup
      {onOpenAutoFocus}
      {onCloseAutoFocus}
      onInteractOutside={(e) => {
        if (e.target instanceof Node && anchor?.contains(e.target)) e.preventDefault();
      }}
      class="z-50 rounded-md bg-pop {flush ? 'p-1.25' : 'p-3'} text-sm leading-normal shadow-pop outline-none"
    >
      {@render children()}
    </Popover.Content>
  </Popover.Portal>
</Popover.Root>
