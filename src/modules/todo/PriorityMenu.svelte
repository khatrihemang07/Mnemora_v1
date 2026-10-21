<!--
  PriorityMenu: the four Priorities as a menu next to `anchor`: P1 red, P2 amber, P3 violet flags and
  "No priority". `current` is ticked; the key 1 to 4 shows beside each. `onpick` gets 1 to 4.
-->
<script lang="ts">
  import Flag from '~icons/lucide/flag';
  import Menu, { type MenuAnchor, type MenuItem } from '../../lib/ui/Menu.svelte';
  import { PRIORITY_NAMES } from './tasks';

  interface Props {
    open: boolean;
    anchor: MenuAnchor | null;
    current: number;
    onpick: (priority: number) => void;
    returnFocus?: HTMLElement | null;
  }
  let { open = $bindable(), anchor, current, onpick, returnFocus = null }: Props = $props();

  const TONES = ['text-danger', 'text-warning', 'text-accent', 'text-faint'];

  const items = $derived<MenuItem[]>(
    [1, 2, 3, 4].map((p) => ({
      label: PRIORITY_NAMES[p] ?? `Priority ${p}`,
      icon: Flag,
      iconClass: TONES[p - 1],
      shortcut: String(p),
      checked: current === p,
      onselect: () => onpick(p),
    })),
  );
</script>

<Menu bind:open {anchor} {items} label="Priority" {returnFocus} />
