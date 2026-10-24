<!--
  RemoteDeleteFollower: when the Task open in the sheet is deleted on another device, the sheet
  closes and a toast says where, with Restore (an ordinary change). Renders nothing; mounted with
  Todo's overlay. Any other Task deleted elsewhere says nothing.
-->
<script lang="ts">
  import { onDestroy, onMount } from 'svelte';
  import { events } from '../../lib/ipc/bindings';
  import { IpcError } from '../../lib/ipc/unwrap';
  import { notifyAction, notifyError } from '../../lib/ui/toast';
  import { invalidateTodo, restoreTask } from './queries';
  import { followRemoteDelete, remoteDeletes } from './remote-delete';
  import { taskSheet } from './task-sheet.svelte';

  let unlisten: (() => void) | undefined;
  let destroyed = false;

  onMount(async () => {
    const stop = await events.todoTaskDeletedEvent.listen((event) => {
      const told = followRemoteDelete(taskSheet.id, event.payload);
      if (!told) return;
      const id = event.payload.task_id;
      remoteDeletes.mark(id);
      taskSheet.close();
      notifyAction(told.message, 'Restore', () => {
        restoreTask(id)
          .catch((e) =>
            notifyError(e instanceof IpcError ? e.message : "Couldn't restore the Task"),
          )
          .finally(() => void invalidateTodo());
      });
    });
    if (destroyed) stop();
    else unlisten = stop;
  });
  onDestroy(() => {
    destroyed = true;
    unlisten?.();
  });
</script>
