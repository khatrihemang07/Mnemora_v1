<!--
  TaskRow: one Task in a list: the diamond to tick it, then its title (wrapping, never cut). Its menu
  (⋯ on hover, right-click, long-press on a phone) holds "Move to list…", which opens a List picker
  next to the row. More actions arrive with the Task sheet.
-->
<script lang="ts">
  import FolderInput from '~icons/lucide/folder-input';
  import Ellipsis from '~icons/lucide/ellipsis';
  import type { Task } from '../../lib/ipc/bindings';
  import { LongPress } from '../../lib/long-press';
  import IconButton from '../../lib/ui/IconButton.svelte';
  import Menu, { type MenuAnchor, type MenuItem } from '../../lib/ui/Menu.svelte';
  import TaskCheckbox from '../../lib/ui/TaskCheckbox.svelte';
  import ListPicker from './ListPicker.svelte';
  import { moveTaskTo } from './task-actions';

  interface Props {
    task: Task;
    ontick: (task: Task) => void;
  }
  let { task, ontick }: Props = $props();

  let row = $state<HTMLElement | null>(null);
  let more = $state<HTMLButtonElement | null>(null);
  let menuOpen = $state(false);
  let menuAnchor = $state.raw<MenuAnchor | null>(null);
  let pickerOpen = $state(false);

  const items: MenuItem[] = [
    {
      label: 'Move to list…',
      icon: FolderInput,
      // The menu hands focus back as it closes; the picker opens after that.
      onselect: () => void Promise.resolve().then(() => (pickerOpen = true)),
    },
  ];

  function openMenu(anchor: MenuAnchor) {
    menuAnchor = anchor;
    menuOpen = true;
  }

  function oncontextmenu(e: MouseEvent) {
    e.preventDefault();
    // The keyboard's menu key reports no position: hang it off the row.
    openMenu(e.clientX === 0 && e.clientY === 0 && row ? row : { x: e.clientX, y: e.clientY });
  }

  const press = new LongPress({ onpress: (x, y) => openMenu({ x, y }) });
  const touchLike = (e: PointerEvent) => e.pointerType === 'touch' || e.pointerType === 'pen';
</script>

<div
  bind:this={row}
  role="presentation"
  {oncontextmenu}
  onpointerdown={(e) => touchLike(e) && !(e.target as Element).closest('button') && press.start(e.clientX, e.clientY)}
  onpointermove={(e) => touchLike(e) && press.move(e.clientX, e.clientY)}
  onpointerup={() => press.cancel()}
  onpointercancel={() => press.cancel()}
  class="group grid grid-cols-[26px_1fr_auto] items-start gap-x-2 rounded-sm px-2 py-[7px] transition-colors duration-(--dur) ease-out
    hover:bg-hover phone:grid-cols-[30px_1fr_auto] phone:px-1.5 phone:py-2.5"
>
  <TaskCheckbox
    checked={task.state !== 'open'}
    label={`Complete ${task.title}`}
    onchange={() => ontick(task)}
    class="mt-px"
  />
  <span class="min-w-0 pt-px leading-[1.45] [overflow-wrap:anywhere]">{task.title}</span>
  <IconButton
    bind:ref={more}
    icon={Ellipsis}
    label="Task actions"
    size="sm"
    onclick={() => openMenu(more as HTMLElement)}
    class="-my-0.5 opacity-0 group-hover:opacity-100 focus-visible:opacity-100 phone:hidden {menuOpen ? 'opacity-100' : ''}"
  />
</div>

<Menu bind:open={menuOpen} anchor={menuAnchor} {items} label="Task actions" returnFocus={row} />

<ListPicker
  bind:open={pickerOpen}
  anchor={more ?? row}
  current={task.list_id}
  onpick={(list) => void moveTaskTo(task, list)}
/>
