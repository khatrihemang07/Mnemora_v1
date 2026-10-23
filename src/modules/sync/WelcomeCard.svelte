<!--
  WelcomeCard: the first thing after turning Sync on, shown on Devices until the first pairing.
  Three lines on how it works, this device's name (prefilled from the computer or phone) and the two
  ways to start pairing; the sheet itself is PairSheet, mounted by the Devices page.
-->
<script lang="ts">
  import type { Component } from 'svelte';
  import GitMerge from '~icons/lucide/git-merge';
  import Plus from '~icons/lucide/plus';
  import ShieldCheck from '~icons/lucide/shield-check';
  import Wifi from '~icons/lucide/wifi';
  import Button from '../../lib/ui/Button.svelte';
  import Card from '../../lib/ui/Card.svelte';
  import DeviceNameField from './DeviceNameField.svelte';
  import { pairing } from './pairing.svelte';

  const lines: { icon: Component<{ class?: string }>; text: string }[] = [
    { icon: Wifi, text: 'Your devices talk directly, on the same Wi-Fi or over Tailscale.' },
    {
      icon: ShieldCheck,
      text: 'Nothing is stored anywhere else. Only devices you pair can connect.',
    },
    {
      icon: GitMerge,
      text: 'Changes merge on their own. If the same text was rewritten on two devices, Sync keeps both and asks you.',
    },
  ];
</script>

<Card>
  <h2 class="font-display text-xl" style="font-weight: var(--display-weight); letter-spacing: var(--display-track)">
    Keep your devices in step
  </h2>
  <ul class="mt-2.5 flex flex-col">
    {#each lines as l (l.text)}
      <li class="flex items-start gap-2.5 border-line py-2 not-first:border-t">
        <l.icon class="mt-0.5 size-4 shrink-0 text-faint" />
        <span>{l.text}</span>
      </li>
    {/each}
  </ul>
  <div class="mt-4 text-sm text-muted">This device’s name</div>
  <DeviceNameField class="mt-1" />
  <div class="mt-4 flex flex-wrap items-center gap-2">
    <Button variant="primary" onclick={() => void pairing.start('show')}><Plus class="size-4" /> Pair a device</Button>
    <Button variant="ghost" onclick={() => void pairing.start('enter')}>Enter a code from another device</Button>
  </div>
</Card>
