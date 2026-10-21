<!--
  Toaster: the single toast host (N5-A). Bottom-right, max 3 visible, 4s, pauses on hover.
  On the phone it sits above the tab bar.
-->
<script lang="ts">
  import { Toaster } from 'svelte-sonner';
  import { viewport } from '../viewport.svelte';
</script>

<Toaster
  position="bottom-right"
  visibleToasts={3}
  duration={4000}
  offset={16}
  mobileOffset={{ bottom: viewport.isPhone ? 'calc(72px + env(safe-area-inset-bottom))' : 16, left: 'calc(16px + env(safe-area-inset-left))', right: 'calc(16px + env(safe-area-inset-right))' }}
  toastOptions={{ class: 'mnemora-toast' }}
/>

<style>
  /* `html` outranks svelte-sonner's own [data-sonner-theme] colours, which ignore our theme. */
  :global(html [data-sonner-toaster][data-sonner-theme]) {
    --normal-bg: var(--pop);
    --normal-text: var(--ink);
    --normal-border: var(--line);
    --border-radius: var(--r2);
    --success-bg: var(--pop);
    --success-text: var(--ink);
    --success-border: var(--line);
    --error-bg: var(--pop);
    --error-text: var(--danger);
    --error-border: var(--danger);
    font-family: var(--f-ui);
  }
  /* A modal (the Task sheet) turns pointer events off on the page; a toast with Undo must stay clickable. */
  :global(html [data-sonner-toast]) {
    pointer-events: auto;
  }
  :global(html [data-sonner-toast][data-styled='true'] [data-button]) {
    background: var(--accent);
    color: var(--on-accent);
  }
</style>
