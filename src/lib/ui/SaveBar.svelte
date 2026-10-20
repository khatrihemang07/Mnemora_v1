<!-- SaveBar (N9-B): slides in while there are unsaved edits. Discard reverts, Save commits. -->
<script lang="ts">
  import Button from './Button.svelte';

  interface Props {
    visible: boolean;
    saving?: boolean;
    onsave: () => void;
    ondiscard: () => void;
  }
  let { visible, saving = false, onsave, ondiscard }: Props = $props();
</script>

<div
  inert={!visible}
  role="region"
  aria-label="Unsaved changes"
  class="pointer-events-none absolute inset-x-0 bottom-0 z-30 flex justify-center p-3 transition-[transform,opacity] duration-(--dur) ease-out
    {visible ? 'translate-y-0 opacity-100' : 'translate-y-full opacity-0'}"
>
  <div
    class="pointer-events-auto flex w-full max-w-xl items-center gap-3 rounded-md bg-pop px-4 py-2.5 shadow-pop"
  >
    <span class="flex-1 text-muted">You have unsaved changes</span>
    <Button variant="ghost" size="sm" onclick={ondiscard} disabled={saving}>Discard</Button>
    <Button variant="primary" size="sm" onclick={onsave} disabled={saving}>
      {saving ? 'Saving…' : 'Save'}
    </Button>
  </div>
</div>
