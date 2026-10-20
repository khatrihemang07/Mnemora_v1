<!-- EntryRow: one Entry on a Page: its time (where it was written) and its text, line breaks kept. -->
<script lang="ts">
  import type { Entry } from '../../lib/ipc/bindings';
  import type { TimeFormat } from '../../lib/time/format';
  import { writtenTime } from './dates';

  let { entry, timeFormat }: { entry: Entry; timeFormat: TimeFormat } = $props();
</script>

<article class="grid gap-1 py-3 min-[720px]:grid-cols-[4rem_1fr] min-[720px]:gap-3">
  <time class="text-xs text-muted tabular-nums min-[720px]:pt-1.5">
    {writtenTime(entry.written_at, entry.written_offset_min, timeFormat)}
  </time>
  <p class="selectable font-display text-base leading-[1.65] break-words whitespace-pre-wrap text-ink">
    {entry.body}
  </p>
</article>
