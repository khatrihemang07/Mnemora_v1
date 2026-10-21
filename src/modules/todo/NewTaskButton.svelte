<!--
  NewTaskButton: the teal "New task" button in the top bar of Todo's pages, with Q beside it. It does
  what Q does: opens the capture box in place on this page, or the floating Quick add where the page
  has no "+ New task" row (Done). Not shown on a phone, which has the rows and the tab bar.
-->
<script lang="ts">
  import Plus from '~icons/lucide/plus';
  import { quickAdd } from './quick-add.svelte';
</script>

<button
  type="button"
  onclick={() => quickAdd.request(true)}
  class="flex h-7 cursor-pointer items-center gap-1.5 rounded-sm bg-hue-todo pr-2 pl-2.5 text-xs font-semibold text-on-accent
    transition-[transform,filter] duration-(--dur) ease-out hover:brightness-110 active:scale-[0.96] phone:hidden"
>
  <Plus class="size-3.5" />
  New task
  <kbd class="ml-0.5 rounded-[4px] bg-[color-mix(in_oklch,var(--on-accent)_22%,transparent)] px-1 font-mono text-[0.68rem] leading-4">Q</kbd>
</button>
