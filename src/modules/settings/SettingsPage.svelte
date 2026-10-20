<!-- SettingsPage (N18-A): section nav + panel; one draft and one SaveBar for all sections. -->
<script lang="ts">
  import { createQuery } from '@tanstack/svelte-query';
  import type { Component } from 'svelte';
  import Boxes from '~icons/lucide/boxes';
  import Database from '~icons/lucide/database';
  import Info from '~icons/lucide/info';
  import Keyboard from '~icons/lucide/keyboard';
  import PanelLeft from '~icons/lucide/panel-left';
  import Palette from '~icons/lucide/palette';
  import SlidersHorizontal from '~icons/lucide/sliders-horizontal';
  import { blockNavigation } from '../../lib/router';
  import { settingsQuery } from '../../lib/settings/queries';
  import { settingsDraft } from '../../lib/settings/store.svelte';
  import ErrorBanner from '../../lib/ui/ErrorBanner.svelte';
  import SaveBar from '../../lib/ui/SaveBar.svelte';
  import SettingsLayout from '../../lib/ui/SettingsLayout.svelte';
  import Skeleton from '../../lib/ui/Skeleton.svelte';
  import { viewport } from '../../lib/viewport.svelte';
  import { moduleSettingsSections } from '../../shell/enabled-modules';
  import { useEnabledModules } from '../../shell/enabled.svelte';
  import { leaveGuard } from './leave-guard.svelte';
  import { save } from './save.svelte';
  import { settingsNav } from './settings-nav.svelte';
  import UnsavedDialog from './UnsavedDialog.svelte';
  import About from './sections/About.svelte';
  import Appearance from './sections/Appearance.svelte';
  import DataStorage from './sections/DataStorage.svelte';
  import General from './sections/General.svelte';
  import ModulesSection from './sections/ModulesSection.svelte';
  import Navigation from './sections/Navigation.svelte';
  import Shortcuts from './sections/Shortcuts.svelte';

  const settings = createQuery(() => settingsQuery);

  const allSections: { id: string; label: string; icon: Component<{ class?: string }>; view: Component; desktopOnly?: true }[] = [
    { id: 'appearance', label: 'Appearance', icon: Palette, view: Appearance },
    { id: 'general', label: 'General', icon: SlidersHorizontal, view: General },
    { id: 'navigation', label: 'Navigation', icon: PanelLeft, view: Navigation },
    { id: 'data', label: 'Data & storage', icon: Database, view: DataStorage },
    { id: 'modules', label: 'Modules', icon: Boxes, view: ModulesSection },
    { id: 'shortcuts', label: 'Shortcuts', icon: Keyboard, view: Shortcuts, desktopOnly: true },
    { id: 'about', label: 'About', icon: Info, view: About },
  ];

  // Enabled Modules add their own sections after the core ones.
  const enabled = useEnabledModules();
  const moduleSections = $derived(moduleSettingsSections(enabled.list));
  const allWithModules = $derived([
    ...allSections,
    ...moduleSections.map((s) => ({ id: `module-${s.id}`, label: s.label, icon: s.icon })),
  ]);

  // Keyboard shortcuts mean nothing on a touch phone.
  const sections = $derived(viewport.isPhone ? allWithModules.filter((s) => !('desktopOnly' in s)) : allWithModules);

  // Desktop always shows a section; the phone starts at the list.
  // A remembered section can vanish (its Module was switched off), so check it still exists.
  const remembered = $derived(sections.some((s) => s.id === settingsNav.section) ? settingsNav.section : null);
  const active = $derived(viewport.isPhone ? remembered : (remembered ?? 'appearance'));
  const View = $derived(allSections.find((s) => s.id === active)?.view);
  const moduleSection = $derived(moduleSections.find((s) => `module-${s.id}` === active));

  $effect(() => {
    if (!settingsDraft.dirty) return;
    return blockNavigation(() => leaveGuard.ask());
  });
</script>

{#if settings.isPending}
  <div class="flex flex-col gap-3 p-6">
    <Skeleton class="h-6 w-40" />
    <Skeleton class="h-24 w-full" />
  </div>
{:else if settings.isError}
  <div class="p-6"><ErrorBanner error={settings.error} onretry={() => settings.refetch()} /></div>
{:else}
  <SettingsLayout {sections} {active} onselect={(id) => (settingsNav.section = id)}>
    {#if save.error}
      <div class="mb-4"><ErrorBanner error={save.error} onretry={() => save.run()} /></div>
    {/if}
    {#if View}
      <View />
    {:else if moduleSection}
      {#await moduleSection.component() then section}<section.default />{/await}
    {/if}
    {#snippet footer()}
      <SaveBar
        visible={settingsDraft.dirty}
        saving={save.saving}
        onsave={() => save.run()}
        ondiscard={() => save.discard()}
      />
    {/snippet}
  </SettingsLayout>
{/if}

<UnsavedDialog />
