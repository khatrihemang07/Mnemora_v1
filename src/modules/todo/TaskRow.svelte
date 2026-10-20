<!-- TaskRow: one Task in a list: the diamond to tick it, then its title (wrapping, never cut). -->
<script lang="ts">
  import type { Task } from '../../lib/ipc/bindings';
  import TaskCheckbox from '../../lib/ui/TaskCheckbox.svelte';

  interface Props {
    task: Task;
    ontick: (task: Task) => void;
  }
  let { task, ontick }: Props = $props();
</script>

<div
  class="grid grid-cols-[26px_1fr] items-start gap-x-2 rounded-sm px-2 py-[7px] transition-colors duration-(--dur) ease-out
    hover:bg-hover phone:grid-cols-[30px_1fr] phone:px-1.5 phone:py-2.5"
>
  <TaskCheckbox
    checked={task.state !== 'open'}
    label={`Complete ${task.title}`}
    onchange={() => ontick(task)}
    class="mt-px"
  />
  <span class="min-w-0 pt-px leading-[1.45] [overflow-wrap:anywhere]">{task.title}</span>
</div>
