<!-- ErrorBanner (N8-A): inline error with Retry and Copy details. A validation error has no Copy details. -->
<script lang="ts">
  import AlertCircle from '~icons/lucide/circle-alert';
  import { IpcError } from '../ipc/unwrap';
  import Button from './Button.svelte';
  import { notify } from './toast';

  interface Props {
    error: unknown;
    onretry?: () => void;
    /** Replaces the error's own message (the details still come from the error). */
    message?: string;
  }
  let { error, onretry, message: override }: Props = $props();

  const message = $derived(
    override ?? (error instanceof Error ? error.message : 'Something went wrong.'),
  );
  /** A plain validation error says what to fix in its sentence; there are no details to copy. */
  const copyable = $derived(!(error instanceof IpcError && error.code === 'validation'));
  const details = $derived(
    error instanceof IpcError
      ? JSON.stringify({ code: error.code, message: error.message, details: error.details }, null, 2)
      : error instanceof Error
        ? (error.stack ?? error.message)
        : String(error),
  );

  async function copy() {
    try {
      await navigator.clipboard.writeText(details);
      notify('Details copied');
    } catch {
      notify('Could not copy details');
    }
  }
</script>

<div
  role="alert"
  class="flex flex-wrap items-center gap-3 rounded-md border border-danger bg-danger-soft px-4 py-3 text-ink"
>
  <AlertCircle class="size-4 shrink-0 text-danger" />
  <span class="min-w-0 flex-1">{message}</span>
  <span class="flex gap-2">
    {#if onretry}<Button size="sm" onclick={onretry}>Retry</Button>{/if}
    {#if copyable}<Button size="sm" variant="ghost" onclick={copy}>Copy details</Button>{/if}
  </span>
</div>
