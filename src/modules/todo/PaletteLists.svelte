<!--
  PaletteLists: gives ⌘K a "Go to ‹List›" action for every List, for as long as Todo is on. It is
  mounted with Todo's overlay, so it renders nothing and disappears with the Module.
-->
<script lang="ts">
  import { createQuery } from '@tanstack/svelte-query';
  import { onDestroy } from 'svelte';
  import { dynamicActions, type ModuleAction } from '../../lib/router';
  import { todoNav } from './nav.svelte';
  import { listsQuery } from './queries';
  import { LIST_ICON } from './view-icons';
  import { listView } from './views';

  const sidebar = createQuery(() => listsQuery());

  $effect(() => {
    const actions: ModuleAction[] = (sidebar.data?.lists ?? []).map(({ list }) => ({
      id: `go-list-${list.id}`,
      label: `Go to ${list.emoji ? `${list.emoji} ` : ''}${list.name}`,
      icon: LIST_ICON,
      run: (ctx) => {
        todoNav.go(listView(list.id));
        ctx.navigate('/todo');
      },
    }));
    dynamicActions.set('todo', actions);
  });
  onDestroy(() => dynamicActions.clear('todo'));
</script>
