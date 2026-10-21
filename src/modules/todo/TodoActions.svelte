<!--
  TodoActions: the buttons at the right of every Todo page's top bar: search (⌘F; pressed while it
  is open) and "New task". On a phone "New task" is left out (the rows and the tab bar add Tasks).
-->
<script lang="ts">
  import Search from '~icons/lucide/search';
  import { shortcutLabel } from '../../lib/platform';
  import IconButton from '../../lib/ui/IconButton.svelte';
  import NewTaskButton from './NewTaskButton.svelte';
  import { todoSearch } from './search.svelte';
</script>

<IconButton
  icon={Search}
  label="Search tasks"
  shortcut={shortcutLabel('F')}
  aria-pressed={todoSearch.active}
  onclick={() => (todoSearch.active ? todoSearch.close() : todoSearch.open())}
/>
<NewTaskButton />
