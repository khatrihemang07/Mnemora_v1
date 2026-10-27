<!-- Switch: on/off toggle. `label` is the accessible name. -->
<script lang="ts">
  import { Switch } from 'bits-ui';
  interface Props {
    checked: boolean;
    label: string;
    onchange: (checked: boolean) => void;
    disabled?: boolean;
  }
  let { checked, label, onchange, disabled = false }: Props = $props();
</script>

<Switch.Root
  {checked}
  {disabled}
  onCheckedChange={onchange}
  aria-label={label}
  class="relative inline-flex h-5 w-9 shrink-0 cursor-pointer items-center rounded-full bg-switch-off
    transition-colors duration-(--dur) ease-out data-[state=checked]:bg-accent disabled:opacity-50"
>
  <Switch.Thumb
    class="pointer-events-none block size-4 translate-x-0.5 rounded-full bg-surface shadow-card
      transition-transform duration-(--dur) ease-out data-[state=checked]:translate-x-4.5"
  />
</Switch.Root>
