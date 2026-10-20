<!--
  EntryRow: one Entry on a Page. Desktop puts the time in a 64px left margin, phone puts it above the
  text. When it was written on another day than its Page, a short date comes with the time (stacked
  above it on desktop). Hovering the time shows the full written timestamp and zone. The text is
  rendered Markdown.
-->
<script lang="ts">
  import type { Entry } from '../../lib/ipc/bindings';
  import type { TimeFormat } from '../../lib/time/format';
  import MarkdownView from '../../lib/ui/MarkdownView.svelte';
  import { editedLabel, fullWritten, writtenDateLabel, writtenTime } from './dates';

  let { entry, timeFormat }: { entry: Entry; timeFormat: TimeFormat } = $props();

  const time = $derived(writtenTime(entry.written_at, entry.written_offset_min, timeFormat));
  const writtenDate = $derived(
    writtenDateLabel(entry.written_at, entry.written_offset_min, entry.page_date),
  );
  const full = $derived(
    fullWritten(entry.written_at, entry.written_offset_min, entry.written_tz, timeFormat),
  );
  // The edit time is shown on the viewer's own clock.
  const edited = $derived(
    entry.edited_at === null
      ? null
      : editedLabel(entry.edited_at, -new Date(entry.edited_at).getTimezoneOffset(), timeFormat),
  );
</script>

<article
  class="relative -mx-2.5 grid gap-0.5 rounded-md px-2.5 py-2 hover:bg-hover min-[720px]:grid-cols-[4rem_1fr] min-[720px]:gap-x-3.5 [&+&]:before:absolute [&+&]:before:inset-x-2.5 [&+&]:before:top-0 [&+&]:before:h-px [&+&]:before:bg-line"
>
  <time class="text-xs text-faint tabular-nums min-[720px]:pt-[3px] min-[720px]:text-[0.78rem]" title={full}>
    {#if writtenDate}
      <span class="mr-1.5 min-[720px]:mr-0 min-[720px]:block min-[720px]:text-[0.65rem] min-[720px]:leading-[1.3] min-[720px]:whitespace-nowrap">
        {writtenDate}
      </span>
    {/if}
    {time}
  </time>
  <div class="min-w-0">
    <MarkdownView source={entry.body} class="break-words" />
    {#if edited}
      <p class="mt-0.5 text-[0.72rem] text-faint" title={edited}>edited</p>
    {/if}
  </div>
</article>
