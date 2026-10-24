<!--
  ListMergeFollower: when Sync merges two Lists with the same name, whoever is looking at the List
  that went away is moved to the one that was kept, with a toast. Renders nothing; mounted with
  Todo's overlay, so it listens for as long as Todo is on. (If Todo is not on screen the view is
  still moved, silently, so coming back does not land on a List that is gone.)
-->
<script lang="ts">
  import { onDestroy, onMount } from 'svelte';
  import { events } from '../../lib/ipc/bindings';
  import { notify } from '../../lib/ui/toast';
  import { followMerge } from './list-merge';
  import { todoNav } from './nav.svelte';

  let unlisten: (() => void) | undefined;
  let destroyed = false;

  onMount(async () => {
    const stop = await events.todoListsMergedEvent.listen((event) => {
      const moved = followMerge(todoNav.view, event.payload);
      if (!moved) return;
      todoNav.go(moved.view);
      if (location.pathname.startsWith('/todo')) notify(moved.message);
    });
    if (destroyed) stop();
    else unlisten = stop;
  });
  onDestroy(() => {
    destroyed = true;
    unlisten?.();
  });
</script>
