<!--
  JournalDeleteFollower: when the Entry open in the editor is deleted on another device, the editor
  closes at once and a toast says where, with Restore, which brings the Entry back with the text
  that was being typed (the same as an open Task sheet). Renders nothing; mounted with the Journal.
  Any other Entry deleted elsewhere says nothing: the feed just refreshes.
-->
<script lang="ts">
  import { onDestroy, onMount } from 'svelte';
  import { events } from '../../lib/ipc/bindings';
  import { IpcError } from '../../lib/ipc/unwrap';
  import { notifyAction, notifyError } from '../../lib/ui/toast';
  import { useEditing } from './editing.svelte';
  import { followEntryDelete, restoreWithDraft } from './entry-remote-delete';
  import { useNav } from './navigation.svelte';
  import { invalidateJournal, restoreEntry, updateEntry } from './queries';

  const editing = useEditing();
  const nav = useNav();

  let unlisten: (() => void) | undefined;
  let destroyed = false;

  onMount(async () => {
    const stop = await events.journalEntryDeletedEvent.listen((event) => {
      const told = followEntryDelete(editing.id, event.payload, nav.today);
      if (!told) return;
      const id = event.payload.entry_id;
      // Drop the session first: it must not try to save into an Entry that is gone.
      const draft = editing.abandon?.() ?? '';
      editing.close();
      void invalidateJournal();
      notifyAction(told.message, 'Restore', () => {
        restoreWithDraft(
          { restore: restoreEntry, update: updateEntry, session: () => crypto.randomUUID() },
          id,
          draft,
        )
          .catch((e) =>
            notifyError(e instanceof IpcError ? e.message : "Couldn't restore the Entry"),
          )
          .finally(() => void invalidateJournal());
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
