<!-- Navigation: sidebar state (a device setting, applies at once) and phone tab chooser. -->
<script lang="ts">
  import ArrowDown from '~icons/lucide/arrow-down';
  import ArrowUp from '~icons/lucide/arrow-up';
  import { navEntries } from '../../../lib/router';
  import { settingsDraft as d } from '../../../lib/settings/store.svelte';
  import Button from '../../../lib/ui/Button.svelte';
  import IconButton from '../../../lib/ui/IconButton.svelte';
  import SegmentedControl from '../../../lib/ui/SegmentedControl.svelte';
  import SettingRow from '../../../lib/ui/SettingRow.svelte';
  import SettingSection from '../../../lib/ui/SettingSection.svelte';
  import { MAX_TABS, moveTab, toggleTab } from '../../../shell/phone-tabs';
  import { modules } from '../../../shell/registry';
  import { shell } from '../../../shell/shell-state.svelte';

  const entries = navEntries(modules);
  const nameOf = (id: string) => entries.find((e) => e.id === id)?.manifest.name ?? id;
  const tabs = $derived(
    ((d.get('core.phone_tabs') as string[]) ?? []).filter((id) => entries.some((e) => e.id === id)),
  );
  const unused = $derived(entries.filter((e) => !tabs.includes(e.id)));
  const setTabs = (next: string[]) => d.set('core.phone_tabs', next);
</script>

<SettingSection title="Navigation">
  <SettingRow label="Sidebar" hint="Applies immediately on this device.">
    <SegmentedControl
      label="Sidebar"
      value={shell.layout.rail ? 'rail' : 'expanded'}
      options={[
        { value: 'expanded', label: 'Expanded' },
        { value: 'rail', label: 'Icon rail' },
      ]}
      onchange={(v) => (shell.layout = { ...shell.layout, rail: v === 'rail' })}
    />
  </SettingRow>
  <SettingRow label="Phone tabs" hint={`Up to ${MAX_TABS}. The rest live under More.`}>
    <div class="flex flex-col gap-2">
      <ul class="flex flex-col gap-1.5" aria-label="Selected tabs">
        {#each tabs as id, i (id)}
          <li class="flex items-center gap-1 rounded-sm border border-line bg-surface-2 py-0.5 pr-1 pl-3">
            <span class="flex-1">{nameOf(id)}</span>
            <IconButton icon={ArrowUp} label={`Move ${nameOf(id)} up`} size="sm" disabled={i === 0}
              onclick={() => setTabs(moveTab(tabs, id, -1))} />
            <IconButton icon={ArrowDown} label={`Move ${nameOf(id)} down`} size="sm" disabled={i === tabs.length - 1}
              onclick={() => setTabs(moveTab(tabs, id, 1))} />
            <Button size="sm" variant="ghost" onclick={() => setTabs(toggleTab(tabs, id))}>Remove</Button>
          </li>
        {/each}
      </ul>
      {#if unused.length}
        <ul class="flex flex-wrap gap-2" aria-label="Available tabs">
          {#each unused as e (e.id)}
            <li>
              <Button size="sm" disabled={tabs.length >= MAX_TABS} onclick={() => setTabs(toggleTab(tabs, e.id))}>
                + {e.manifest.name}
              </Button>
            </li>
          {/each}
        </ul>
      {/if}
    </div>
  </SettingRow>
</SettingSection>
