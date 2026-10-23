<!--
  DeviceNameField: this device's name, as the other devices will see it. It saves when you leave the
  field or press Enter; Rust checks it (1 to 40 characters) and its message shows under the field.
-->
<script lang="ts">
  import { createQuery } from '@tanstack/svelte-query';
  import { commands } from '../../lib/ipc/bindings';
  import { unwrap } from '../../lib/ipc/unwrap';
  import Input from '../../lib/ui/Input.svelte';
  import { refreshSync, syncQuery } from './queries';

  let { class: cls = '' }: { class?: string } = $props();

  const sync = createQuery(() => syncQuery);
  // null: show the saved name. Text stays here, unsaved, while it is refused.
  let text = $state<string | null>(null);
  let error = $state<string | null>(null);
  const shown = $derived(text ?? sync.data?.name ?? '');

  async function commit() {
    if (text === null) return;
    try {
      await unwrap(commands.syncRename(text));
      text = null;
      error = null;
      await refreshSync();
    } catch (e) {
      error = e instanceof Error ? e.message : 'Could not rename this device';
    }
  }
</script>

<div class={cls}>
  <Input
    label="This device’s name"
    bind:value={() => shown, (v) => { text = v; error = null; }}
    invalid={error !== null}
    onblur={() => void commit()}
    onkeydown={(e) => e.key === 'Enter' && void commit()}
  />
  {#if error}<p role="alert" class="mt-1 text-sm text-danger">{error}</p>{/if}
</div>
