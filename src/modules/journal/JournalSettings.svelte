<!--
  JournalSettings: the Journal's section in Settings. Writing font, Text width and Page summaries are
  Preferences, so they go through the shared draft and its SaveBar like every other Setting.
  Spellcheck is a Device setting (this device only, never synced): like the sidebar layout it is
  saved at once through its own command, not through the draft.
-->
<script lang="ts">
  import { createQuery } from '@tanstack/svelte-query';
  import { commands } from '../../lib/ipc/bindings';
  import { queryClient } from '../../lib/ipc/query-client';
  import { unwrap } from '../../lib/ipc/unwrap';
  import { deviceQuery } from '../../lib/settings/queries';
  import { settingsDraft as d } from '../../lib/settings/store.svelte';
  import SegmentedControl from '../../lib/ui/SegmentedControl.svelte';
  import SettingRow from '../../lib/ui/SettingRow.svelte';
  import SettingSection from '../../lib/ui/SettingSection.svelte';
  import Switch from '../../lib/ui/Switch.svelte';
  import { notifyError } from '../../lib/ui/toast';
  import { type JournalFont, type JournalWidth, readJournal } from './settings';

  // The draft holds unsaved edits, so the controls show what is chosen, saved or not.
  const prefs = $derived(readJournal(d.merged()));
  const device = createQuery(() => deviceQuery);
  const spellcheck = $derived(device.data?.spellcheck ?? true);

  async function setSpellcheck(enabled: boolean) {
    try {
      await unwrap(commands.deviceSetSpellcheck(enabled));
    } catch (e) {
      notifyError(e instanceof Error ? e.message : 'Could not change spellcheck');
    }
    // The device query never goes stale by itself, so ask for the new value.
    await queryClient.invalidateQueries({ queryKey: deviceQuery.queryKey });
  }
</script>

<SettingSection title="Journal">
  <SettingRow label="Writing font" hint="For reading and writing Entries.">
    <SegmentedControl
      label="Writing font"
      value={prefs.font}
      options={[
        { value: 'serif', label: 'Serif' },
        { value: 'sans', label: 'Sans' },
      ]}
      onchange={(v: JournalFont) => d.set('journal.font', v)}
    />
  </SettingRow>
  <SettingRow label="Text width" hint="How wide the column of text is. It follows Text size.">
    <SegmentedControl
      label="Text width"
      value={prefs.width}
      options={[
        { value: 'narrow', label: 'Narrow' },
        { value: 'default', label: 'Default' },
        { value: 'wide', label: 'Wide' },
      ]}
      onchange={(v: JournalWidth) => d.set('journal.width', v)}
    />
  </SettingRow>
  <SettingRow label="Page summaries" hint="Show “3 entries · 120 words” in each Page header.">
    <Switch
      label="Page summaries"
      checked={prefs.pageSummary}
      onchange={(on) => d.set('journal.page_summary', on)}
    />
  </SettingRow>
  <SettingRow label="Spellcheck" hint="Underline misspelled words while writing. This device only.">
    <Switch label="Spellcheck" checked={spellcheck} onchange={(on) => void setSpellcheck(on)} />
  </SettingRow>
</SettingSection>
