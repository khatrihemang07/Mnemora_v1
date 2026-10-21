<!--
  TodoSettings: Todo's section in Settings. Both rows are Preferences, so they go through the shared
  draft and its SaveBar like every other Setting. (Only the sidebar's fold state is a Device
  setting, and it is saved by the sidebar itself.)
-->
<script lang="ts">
  import { settingsDraft as d } from '../../lib/settings/store.svelte';
  import Select from '../../lib/ui/Select.svelte';
  import SettingRow from '../../lib/ui/SettingRow.svelte';
  import SettingSection from '../../lib/ui/SettingSection.svelte';
  import Switch from '../../lib/ui/Switch.svelte';
  import { DONE_IN_LISTS_OPTIONS, DONE_IN_LISTS_PREF } from './arrange';
  import { PARSE_DATES_PREF, readTodo } from './settings';

  // The draft holds unsaved edits, so the controls show what is chosen, saved or not.
  const prefs = $derived(readTodo(d.merged()));
</script>

<SettingSection title="Todo">
  <SettingRow
    label="Recognise dates in titles"
    hint="Reads “tomorrow 6pm”, “every Monday”, #List and p1–p4 as you type a Task. The words stay in the title."
  >
    <Switch
      label="Recognise dates in titles"
      checked={prefs.parseDates}
      onchange={(on) => d.set(PARSE_DATES_PREF, on)}
    />
  </SettingRow>
  <SettingRow label="Done Tasks in Lists" hint="What a List page does with the Tasks you finished.">
    <Select
      label="Done Tasks in Lists"
      value={prefs.doneInLists}
      options={[...DONE_IN_LISTS_OPTIONS]}
      onchange={(v) => d.set(DONE_IN_LISTS_PREF, v)}
    />
  </SettingRow>
</SettingSection>
