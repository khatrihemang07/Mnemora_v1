<!--
  AppShell: desktop = resizable sidebar + main; phone = main + bottom tab bar.
  Wires the app-wide effects (appearance, sidebar persistence, shortcuts) from small modules.
-->
<script lang="ts">
  import { createQuery } from '@tanstack/svelte-query';
  import PanelLeft from '~icons/lucide/panel-left';
  import Search from '~icons/lucide/search';
  import { RouterOutlet } from '../lib/router';
  import { isMac, shortcutLabel } from '../lib/platform';
  import { appInfoQuery } from '../lib/settings/queries';
  import CommandPalette from '../lib/ui/CommandPalette.svelte';
  import IconButton from '../lib/ui/IconButton.svelte';
  import Kbd from '../lib/ui/Kbd.svelte';
  import { pageHeader } from '../lib/ui/page-header.svelte';
  import ResizablePanes from '../lib/ui/ResizablePanes.svelte';
  import Toaster from '../lib/ui/Toaster.svelte';
  import TooltipProvider from '../lib/ui/TooltipProvider.svelte';
  import { viewport } from '../lib/viewport.svelte';
  import { useAppearance } from './appearance.svelte';
  import { router } from './router';
  import Sidebar from './Sidebar.svelte';
  import { paletteItems, runShortcut } from './shell-commands';
  import { shell } from './shell-state.svelte';
  import { useSidebarPersistence } from './sidebar-persist.svelte';
  import { resolveShortcut } from './shortcuts';
  import { MAX_WIDTH, RAIL_WIDTH, settleDrag, shownWidth } from './sidebar-layout';
  import TabBar from './TabBar.svelte';
  import TitleBar from './TitleBar.svelte';

  createQuery(() => appInfoQuery);
  useAppearance();
  useSidebarPersistence();

  function onKeydown(e: KeyboardEvent) {
    const action = resolveShortcut(e, isMac);
    if (!action) return;
    e.preventDefault();
    runShortcut(action);
  }
</script>

<svelte:window onkeydown={onKeydown} />

<TooltipProvider>
  <div class="flex h-full flex-col bg-bg text-ink">
    {#if viewport.isPhone}
      <main class="min-h-0 flex-1 pt-[env(safe-area-inset-top)]">
        <RouterOutlet pathname={router.pathname} />
      </main>
      <TabBar />
    {:else if shell.layoutReady}
      <ResizablePanes
        width={shownWidth(shell.layout)}
        minWidth={RAIL_WIDTH}
        maxWidth={MAX_WIDTH}
        rail={shell.layout.rail}
        onResize={(px) => (shell.layout = settleDrag(px, shell.layout))}
        onReset={() => shell.resetSidebar()}
      >
        {#snippet sidebar()}
          <Sidebar />
        {/snippet}
        {#snippet main()}
          <div class="flex h-full min-w-0 flex-col">
            <TitleBar controls bar>
              <div class="pl-2 {isMac && shell.layout.rail ? 'pl-4' : ''}">
                <IconButton
                  icon={PanelLeft}
                  label="Toggle sidebar"
                  shortcut={shortcutLabel('\\')}
                  onclick={() => shell.toggleSidebar()}
                />
              </div>
              {#if pageHeader.current}
                <h1
                  class="ml-2 truncate font-display text-lg"
                  style="font-weight: var(--display-weight); letter-spacing: var(--display-track)"
                >
                  {pageHeader.current.title}
                </h1>
              {/if}
              {#snippet trailing()}
                <div class="flex items-center gap-2 pr-3">
                  {@render pageHeader.current?.actions?.()}
                  <button
                    type="button"
                    onclick={() => (shell.paletteOpen = true)}
                    class="flex h-7 cursor-pointer items-center gap-2 rounded-sm border border-line bg-surface px-2.5 text-sm text-faint
                      transition-[transform,background-color] duration-(--dur) ease-out hover:bg-hover active:scale-[0.96]"
                  >
                    <Search class="size-3.5" />
                    Search…
                    <Kbd keys={shortcutLabel('K')} />
                  </button>
                </div>
              {/snippet}
            </TitleBar>
            <main class="min-h-0 flex-1">
              <RouterOutlet pathname={router.pathname} />
            </main>
          </div>
        {/snippet}
      </ResizablePanes>
    {/if}
  </div>
  <CommandPalette open={shell.paletteOpen} items={paletteItems} onclose={() => (shell.paletteOpen = false)} />
  <Toaster />
</TooltipProvider>
