<!-- General: display name and date/time formats. -->
<script lang="ts">
  import { createQuery } from '@tanstack/svelte-query';
  import { appInfoQuery } from '../../../lib/settings/queries';
  import { settingsDraft as d } from '../../../lib/settings/store.svelte';
  import type { DateFormat, TimeFormat } from '../../../lib/time/format';
  import Input from '../../../lib/ui/Input.svelte';
  import SegmentedControl from '../../../lib/ui/SegmentedControl.svelte';
  import SettingRow from '../../../lib/ui/SettingRow.svelte';
  import SettingSection from '../../../lib/ui/SettingSection.svelte';

  const info = createQuery(() => appInfoQuery);
  const name = $derived(String(d.get('core.display_name') ?? ''));
</script>

<SettingSection title="General">
  <SettingRow
    label="Display name"
    hint={`Used in the greeting. Leave empty to use ${info.data?.os_first_name ?? 'your system name'}.`}
  >
    <Input
      label="Display name"
      value={name}
      maxlength={60}
      placeholder={info.data?.os_first_name ?? ''}
      oninput={(e) => d.set('core.display_name', e.currentTarget.value)}
    />
  </SettingRow>
  <SettingRow label="Time format">
    <SegmentedControl
      label="Time format"
      value={d.get('core.time_format') as TimeFormat}
      options={[
        { value: '24h', label: '24-hour' },
        { value: '12h', label: '12-hour' },
      ]}
      onchange={(v) => d.set('core.time_format', v)}
    />
  </SettingRow>
  <SettingRow label="Date format">
    <SegmentedControl
      label="Date format"
      value={d.get('core.date_format') as DateFormat}
      options={[
        { value: 'long', label: 'Long' },
        { value: 'short', label: 'Short' },
        { value: 'iso', label: 'ISO' },
      ]}
      onchange={(v) => d.set('core.date_format', v)}
    />
  </SettingRow>
  <SettingRow label="Week starts on">
    <SegmentedControl
      label="Week starts on"
      value={d.get('core.week_start') as 'mon' | 'sun'}
      options={[
        { value: 'mon', label: 'Monday' },
        { value: 'sun', label: 'Sunday' },
      ]}
      onchange={(v) => d.set('core.week_start', v)}
    />
  </SettingRow>
</SettingSection>
