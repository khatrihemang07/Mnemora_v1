<!-- Sidebar: Search entry, Modules, then Settings at the bottom, plus Environment and version. -->
<script lang="ts">
  import { createQuery } from '@tanstack/svelte-query';
  import Search from '~icons/lucide/search';
  import { navEntries } from '../lib/router';
  import { shortcutLabel } from '../lib/platform';
  import { appInfoQuery } from '../lib/settings/queries';
  import AppMark from '../lib/ui/AppMark.svelte';
  import EnvBadge from '../lib/ui/EnvBadge.svelte';
  import NavItem from './NavItem.svelte';
  import TitleBar from './TitleBar.svelte';
  import { modules } from './registry';
  import { router } from './router';
  import { shell } from './shell-state.svelte';

  const info = createQuery(() => appInfoQuery);
  const entries = navEntries(modules);
  const main = entries.filter((e) => e.manifest.nav.placement !== 'bottom');
  const bottom = entries.filter((e) => e.manifest.nav.placement === 'bottom');
  const rail = $derived(shell.layout.rail);

  const hint = (digit: number | null) => (digit ? shortcutLabel(String(digit)) : undefined);
</script>

<nav aria-label="Main" class="flex h-full flex-col bg-side">
  <TitleBar />
  <div class="flex items-center gap-2.5 px-4 pb-3 {rail ? 'justify-center px-0' : ''}">
    <AppMark />
    {#if !rail}
      <span class="font-display text-base" style="font-weight: var(--display-weight)">Mnemora</span>
    {/if}
  </div>
  <div class="flex min-h-0 flex-1 flex-col gap-0.5 overflow-y-auto px-2 pb-2">
    <NavItem
      label="Search"
      icon={Search}
      {rail}
      shortcut={shortcutLabel('K')}
      onclick={() => (shell.paletteOpen = true)}
    />
    {#if !rail}
      <div class="px-2.5 pt-3 pb-1 text-xs font-medium text-faint">Modules</div>
    {:else}
      <div class="my-1.5 h-px bg-line"></div>
    {/if}
    {#each main as e (e.id)}
      <NavItem
        label={e.manifest.name}
        icon={e.manifest.icon}
        href={router.href(e.path)}
        active={router.isActive(e.path)}
        accentVar={e.manifest.accentVar}
        shortcut={hint(e.manifest.nav.shortcut)}
        {rail}
      />
    {/each}
    <div class="flex-1"></div>
    {#each bottom as e (e.id)}
      <NavItem
        label={e.manifest.name}
        icon={e.manifest.icon}
        href={router.href(e.path)}
        active={router.isActive(e.path)}
        accentVar={e.manifest.accentVar}
        shortcut={shortcutLabel(',')}
        {rail}
      />
    {/each}
  </div>
  <div class="flex items-center gap-2 px-3.5 py-2.5 text-xs text-faint {rail ? 'justify-center px-0' : ''}">
    {#if info.data}
      <EnvBadge env={info.data.environment} compact={rail} />
      {#if !rail}<span>v{info.data.version}</span>{/if}
    {/if}
  </div>
</nav>
