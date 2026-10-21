<!-- ListGlyph: a List's emoji, or the neutral list icon when it has none. A fixed square so names line up. -->
<script lang="ts">
  import { LIST_ICON } from './view-icons';

  let { emoji, class: cls = '' }: { emoji: string | null; class?: string } = $props();
</script>

<span aria-hidden="true" class="grid size-[1.15rem] shrink-0 place-items-center text-center leading-none {cls}">
  {#if emoji}
    <span class="text-base">{emoji}</span>
  {:else}
    <LIST_ICON class="size-4 text-muted" />
  {/if}
</span>
