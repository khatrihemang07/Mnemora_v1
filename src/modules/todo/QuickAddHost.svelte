<!--
  QuickAddHost: the floating Quick add, mounted by the shell for as long as Todo is on. Q opens it
  anywhere outside Todo's own pages (and inside Todo where the page has no "+ New task" row): the
  same capture box as the inline one, in a centred panel (a bottom sheet on a phone), adding to the
  Inbox unless the text says otherwise; Home's Add opens it due today. The Task is added and the box closes, and the toast names
  the Task and where it went (its day when it has one). On a phone the + button opens it for the view
  on show (Today: due today, Upcoming: tomorrow, a List: in it). Esc closes it.
-->
<script lang="ts">
  import Sheet from '../../lib/ui/Sheet.svelte';
  import { notify } from '../../lib/ui/toast';
  import CaptureBox from './CaptureBox.svelte';
  import { followCoreSettings } from './prefs.svelte';
  import { quickAdd } from './quick-add.svelte';
  import { todoClock } from './clock.svelte';
  import { landedIn } from './dates';
  import { quickAddedMessage } from './tasks';

  // The box reads the Week start and the recognition Preference, even outside Todo's pages.
  followCoreSettings();

  let guard = $state({ escape: false });
</script>

{#if quickAdd.floating}
  <Sheet
    open
    centered
    bare
    title="Quick add"
    onclose={() => quickAdd.close()}
    guardEscape={() => guard.escape}
  >
    <CaptureBox
      class="bg-transparent shadow-none"
      bind:guard
      quick
      date={quickAdd.date}
      listId={quickAdd.listId}
      placeholder={quickAdd.date ? 'Quick add, due ' + (quickAdd.date === todoClock.today ? 'today' : 'tomorrow') : 'Quick add to Inbox'}
      onclose={() => quickAdd.close()}
      onadded={(task, where) => {
        quickAdd.close();
        notify(quickAddedMessage(task.title, landedIn(task, where === 'Inbox' ? null : where, todoClock.today)));
      }}
    />
  </Sheet>
{/if}
