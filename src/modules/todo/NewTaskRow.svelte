<!--
  NewTaskRow: "+ New task" at the end of the list. Click (or Enter) turns it into the capture box in
  place; Enter adds the Task and keeps the box open for the next, Esc or clicking away closes it
  (what was typed is dropped). With a `listId` the Task is added to that List instead of the Inbox;
  with a `date` it is due that day (Today and each Upcoming day add this way). `label` renames the
  row. The box reads dates, Lists, Priorities and Repeats out of what is typed (see CaptureBox). Q
  and the top bar's "New task" open the first of these rows on the page.
-->
<script lang="ts">
  import { tick } from 'svelte';
  import Plus from '~icons/lucide/plus';
  import CaptureBox from './CaptureBox.svelte';

  interface Props {
    listId?: string | null;
    /** `YYYY-MM-DD`: the day a Task added here is due. */
    date?: string | null;
    label?: string;
  }
  let { listId = null, date = null, label = 'New task' }: Props = $props();

  let open = $state(false);
  let opener = $state<HTMLButtonElement | null>(null);

  function close(returnFocus: boolean) {
    open = false;
    if (returnFocus) void tick().then(() => opener?.focus());
  }
</script>

{#if open}
  <CaptureBox class="-mx-0.5 my-1.5 phone:mx-0" {listId} {date} onclose={(reason) => close(reason === 'escape')} />
{:else}
  <button
    bind:this={opener}
    type="button"
    data-new-task
    onclick={() => (open = true)}
    class="grid w-full cursor-text grid-cols-[26px_1fr] items-center gap-x-2 rounded-sm px-2 py-[7px] text-left text-faint
      transition-colors duration-(--dur) ease-out hover:bg-hover hover:text-muted phone:grid-cols-[30px_1fr] phone:px-1.5 phone:py-2.5"
  >
    <Plus class="size-4 justify-self-center" />
    <span>{label}</span>
  </button>
{/if}
