<!-- WindowControls: minimize / maximize / close for Windows and Linux (no native frame). -->
<script lang="ts">
  import Minus from '~icons/lucide/minus';
  import Square from '~icons/lucide/square';
  import X from '~icons/lucide/x';

  async function act(action: 'minimize' | 'toggleMaximize' | 'close') {
    const { getCurrentWindow } = await import('@tauri-apps/api/window');
    await getCurrentWindow()[action]();
  }

  const base =
    'grid h-full w-11 cursor-pointer place-items-center text-muted transition-colors duration-(--dur) ease-out';
</script>

<div class="flex h-full">
  <button type="button" aria-label="Minimize" class="{base} hover:bg-hover hover:text-ink" onclick={() => act('minimize')}>
    <Minus class="size-4" />
  </button>
  <button type="button" aria-label="Maximize" class="{base} hover:bg-hover hover:text-ink" onclick={() => act('toggleMaximize')}>
    <Square class="size-3.5" />
  </button>
  <button type="button" aria-label="Close" class="{base} hover:bg-danger hover:text-on-danger" onclick={() => act('close')}>
    <X class="size-4" />
  </button>
</div>
