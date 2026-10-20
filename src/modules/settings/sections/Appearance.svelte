<!-- Appearance: theme, density, text size, reduce motion. Theme/density/text size preview live. -->
<script lang="ts">
  import Monitor from '~icons/lucide/monitor';
  import Moon from '~icons/lucide/moon';
  import MoonStar from '~icons/lucide/moon-star';
  import Sun from '~icons/lucide/sun';
  import type { Density, TextSize } from '../../../lib/settings/core';
  import { settingsDraft as d } from '../../../lib/settings/store.svelte';
  import type { ThemePref } from '../../../lib/theme/resolve';
  import SegmentedControl from '../../../lib/ui/SegmentedControl.svelte';
  import SettingRow from '../../../lib/ui/SettingRow.svelte';
  import SettingSection from '../../../lib/ui/SettingSection.svelte';
  import Switch from '../../../lib/ui/Switch.svelte';
</script>

<SettingSection title="Appearance">
  <SettingRow label="Theme" hint="Follow the system or choose. True black saves battery on OLED screens.">
    <SegmentedControl
      label="Theme"
      value={d.get('core.theme') as ThemePref}
      options={[
        { value: 'light', label: 'Light', icon: Sun },
        { value: 'dark', label: 'Dark', icon: Moon },
        { value: 'system', label: 'System', icon: Monitor },
        { value: 'oled', label: 'True black', icon: MoonStar },
      ]}
      onchange={(v) => d.set('core.theme', v)}
    />
  </SettingRow>
  <SettingRow label="Density" hint="Compact fits more on screen. Phones always use touch sizes.">
    <SegmentedControl
      label="Density"
      value={d.get('core.density') as Density}
      options={[
        { value: 'comfortable', label: 'Comfortable' },
        { value: 'compact', label: 'Compact' },
      ]}
      onchange={(v) => d.set('core.density', v)}
    />
  </SettingRow>
  <SettingRow label="Text size">
    <SegmentedControl
      label="Text size"
      value={d.get('core.text_size') as TextSize}
      options={[
        { value: 's', label: 'Small' },
        { value: 'm', label: 'Medium' },
        { value: 'l', label: 'Large' },
      ]}
      onchange={(v) => d.set('core.text_size', v)}
    />
  </SettingRow>
  <SettingRow label="Reduce motion" hint="Turns off animations and transitions.">
    <Switch
      label="Reduce motion"
      checked={d.get('core.reduce_motion') === true}
      onchange={(v) => d.set('core.reduce_motion', v)}
    />
  </SettingRow>
</SettingSection>
