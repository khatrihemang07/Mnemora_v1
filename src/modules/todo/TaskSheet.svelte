<!--
  TaskSheet: the open Task in a centred sheet (a bottom sheet on a phone), at `/todo?task=‹id›`.
  Esc, a click outside, ✕ and ⌘↵ close it; Android Back too. The sheet's life is the Task's id in the
  URL; the body below is remounted per Task, so each opening starts a new editing session.
-->
<script lang="ts">
  import Sheet from '../../lib/ui/Sheet.svelte';
  import TaskSheetBody from './TaskSheetBody.svelte';
  import { taskSheet } from './task-sheet.svelte';

  let { id }: { id: string } = $props();
</script>

<Sheet open title="Task" centered bare tall autoFocus={false} onclose={() => taskSheet.close()}>
  {#key id}
    <TaskSheetBody {id} />
  {/key}
</Sheet>
