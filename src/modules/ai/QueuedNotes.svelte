<!--
  The notes typed while a Run answers, waiting above the question box. Each can be sent now (the
  Step in progress ends and the note goes next) or taken back. Notes left over when a Run ended
  wait here too, and "Send now" then asks them as a new question.
-->
<script lang="ts">
  import CornerDownRight from '~icons/lucide/corner-down-right';
  import X from '~icons/lucide/x';
  import type { QueuedNote } from '../../lib/ipc/bindings';
  import Button from '../../lib/ui/Button.svelte';
  import IconButton from '../../lib/ui/IconButton.svelte';

  interface Props {
    notes: readonly QueuedNote[];
    onsendnow: (note: QueuedNote) => void;
    onremove: (note: QueuedNote) => void;
  }

  let { notes, onsendnow, onremove }: Props = $props();
</script>

{#if notes.length > 0}
  <ul class="mx-auto flex w-full max-w-[42.5rem] flex-col gap-1 pb-2" aria-label="Notes waiting">
    {#each notes as note (note.id)}
      <li class="flex items-center gap-2 rounded-md border border-dashed border-line-strong py-1.5 pr-2 pl-2.5 text-sm text-muted">
        <CornerDownRight class="size-4 shrink-0" />
        <span class="min-w-0 flex-1 truncate">{note.text}</span>
        <Button variant="ghost" size="sm" onclick={() => onsendnow(note)}>Send now</Button>
        <IconButton icon={X} label="Remove note" size="xs" variant="ghost" onclick={() => onremove(note)} />
      </li>
    {/each}
  </ul>
{/if}
