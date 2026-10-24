<!--
  DeviceNameField: this device's name, as the other devices will see it. It saves when you leave the
  field or press Enter, and Esc brings the saved name back; Rust checks it (1 to 40 characters) and its message shows under the field.
-->
<script lang="ts">
  import { createQuery } from '@tanstack/svelte-query';
  import { commands } from '../../lib/ipc/bindings';
  import { unwrap } from '../../lib/ipc/unwrap';
  import Input from '../../lib/ui/Input.svelte';
  import { DeviceRename } from './device-rename.svelte';
  import { refreshSync, syncQuery } from './queries';

  let { class: cls = '' }: { class?: string } = $props();

  const sync = createQuery(() => syncQuery);
  const saved = $derived(sync.data?.name ?? '');
  // The same draft as the device sheet's rename: text stays while it is refused, Esc brings the saved name back.
  const rename = new DeviceRename(async (name) => {
    await unwrap(commands.syncRename(name));
    await refreshSync();
  });
  const shown = $derived(rename.editing ? rename.text : saved);
</script>

<div class={cls}>
  <Input
    label="This device’s name"
    bind:value={() => shown, (v) => { if (!rename.editing) rename.start(saved); rename.type(v); }}
    invalid={rename.error !== null}
    onblur={() => void rename.commit(saved)}
    onkeydown={(e) => {
      if (e.key === 'Enter') {
        e.preventDefault();
        void rename.commit(saved);
      } else if (e.key === 'Escape' && rename.editing) {
        e.stopPropagation();
        rename.cancel();
      }
    }}
  />
  {#if rename.error}<p role="alert" class="mt-1 text-sm text-danger">{rename.error}</p>{/if}
</div>
