<!--
  DeleteListDialog: deleting a List that has Tasks asks what becomes of them: delete them too (the
  default), or move them to the Inbox or another List. One of the two deliberate exceptions to
  "Undo, not confirm"; Undo follows either way. The choice goes to `onconfirm` as Rust's DeleteChoice.
-->
<script lang="ts">
  import type { DeleteChoice, ListEntry, TodoList } from '../../lib/ipc/bindings';
  import Button from '../../lib/ui/Button.svelte';
  import Dialog from '../../lib/ui/Dialog.svelte';
  import SegmentedControl from '../../lib/ui/SegmentedControl.svelte';
  import Select from '../../lib/ui/Select.svelte';
  import { quoted } from './tasks';

  interface Props {
    /** The List being deleted; the Dialog is open while this is set. */
    entry: ListEntry | null;
    /** Every other List, for "move to". */
    others: TodoList[];
    onconfirm: (choice: DeleteChoice, targetName?: string) => void;
    onclose: () => void;
  }
  let { entry, others, onconfirm, onclose }: Props = $props();

  const INBOX = 'inbox';
  let mode = $state<'delete' | 'move'>('delete');
  let target = $state(INBOX);

  // Each opening starts on the default.
  $effect(() => {
    if (entry) {
      mode = 'delete';
      target = INBOX;
    }
  });

  const options = $derived([
    { value: INBOX, label: 'Inbox' },
    ...others.map((l) => ({ value: l.id, label: l.name })),
  ]);
  const count = $derived(entry?.task_count ?? 0);
  const noun = $derived(count === 1 ? 'Task' : 'Tasks');

  function confirm() {
    if (mode === 'delete') onconfirm({ kind: 'delete_tasks' });
    else if (target === INBOX) onconfirm({ kind: 'move_to_inbox' });
    else
      onconfirm(
        { kind: 'move_to_list', list_id: target },
        others.find((l) => l.id === target)?.name,
      );
  }
</script>

<Dialog
  open={entry !== null}
  title={entry ? `Delete ${quoted(entry.list.name)}?` : 'Delete list'}
  description={`It has ${count} ${noun}. What should happen to ${count === 1 ? 'it' : 'them'}?`}
  {onclose}
>
  <div class="flex flex-col gap-3">
    <SegmentedControl
      value={mode}
      label="What happens to the Tasks"
      options={[
        { value: 'delete', label: `Delete the ${noun} too` },
        { value: 'move', label: 'Move to…' },
      ]}
      onchange={(v) => (mode = v)}
    />
    {#if mode === 'move'}
      <Select value={target} {options} label="Move the Tasks to" onchange={(v) => (target = v)} />
    {/if}
  </div>
  {#snippet actions()}
    <Button variant="ghost" onclick={onclose}>Cancel</Button>
    <Button variant="danger" onclick={confirm}>Delete list</Button>
  {/snippet}
</Dialog>
