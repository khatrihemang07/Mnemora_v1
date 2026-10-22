<!--
  AppShell: desktop = resizable sidebar + main; phone = main + bottom tab bar.
  Wires the app-wide effects (appearance, sidebar persistence, shortcuts) from small modules.
-->
<script lang="ts">
  import { createQuery } from '@tanstack/svelte-query';
  import PanelLeft from '~icons/lucide/panel-left';
  import Search from '~icons/lucide/search';
  import { RouterOutlet } from '../lib/router';
  import { moduleContext } from './module-context';
  import { bareKeyFires, overlayOpenIn } from '../lib/bare-key';
  import { isMac, shortcutLabel } from '../lib/platform';
  import { appInfoQuery } from '../lib/settings/queries';
  import CommandPalette from '../lib/ui/CommandPalette.svelte';
  import IconButton from '../lib/ui/IconButton.svelte';
  import Kbd from '../lib/ui/Kbd.svelte';
  import { pageHeader } from '../lib/ui/page-header.svelte';
  import ResizablePanes from '../lib/ui/ResizablePanes.svelte';
  import Toaster from '../lib/ui/Toaster.svelte';
  import TooltipProvider from '../lib/ui/TooltipProvider.svelte';
  import { keyboard } from '../lib/keyboard.svelte';
  import { viewport } from '../lib/viewport.svelte';
  import { useAppearance } from './appearance.svelte';
  import { bareKeyFor, redirectForDisabled } from './enabled-modules';
  import { useEnabledModules } from './enabled.svelte';
  import ModuleOverlays from './ModuleOverlays.svelte';
  import { modules } from './registry';
  import { router } from './router';
  import Sidebar from './Sidebar.svelte';
  import { paletteItemsFor, runShortcut } from './shell-commands';
  import { shell } from './shell-state.svelte';
  import { useSidebarPersistence } from './sidebar-persist.svelte';
  import { resolveShortcut } from './shortcuts';
  import { MAX_WIDTH, RAIL_WIDTH, settleDrag, shownWidth } from './sidebar-layout';
  import TabBar from './TabBar.svelte';
  import TitleBar from './TitleBar.svelte';

  const info = createQuery(() => appInfoQuery);
  $effect(() => {
    pageHeader.env = info.data?.environment ?? null;
  });
  useAppearance();
  useSidebarPersistence();
  const enabled = useEnabledModules();
  const paletteItems = $derived(paletteItemsFor(enabled.list));

  // A disabled Module's URL (bookmark, history, a switch-off elsewhere) falls back to Home.
  $effect(() => {
    if (!enabled.ready) return;
    const to = redirectForDisabled(router.pathname, modules, enabled.list);
    if (to) void router.navigate(to);
  });

  function onKeydown(e: KeyboardEvent) {
    const action = resolveShortcut(e, isMac);
    if (action) {
      e.preventDefault();
      runShortcut(action, enabled.list);
      return;
    }
    // A bare letter (Q) is a shortcut only when nothing is being typed into and nothing is open.
    const bare = bareKeyFor(enabled.list, e.key);
    if (bare && bareKeyFires(e, bare.key, { target: e.target, overlayOpen: overlayOpenIn(document) })) {
      e.preventDefault();
      bare.run(moduleContext);
    }
  }
</script>

<svelte:window onkeydown={onKeydown} />

<TooltipProvider>
  <div
    class="flex h-full flex-col bg-bg text-ink {viewport.isPhone ? 'pl-[env(safe-area-inset-left)] pr-[env(safe-area-inset-right)]' : ''}"
    style:padding-bottom={viewport.isPhone && keyboard.open ? `${keyboard.inset}px` : undefined}
  >
    {#if viewport.isPhone}
      <main class="min-h-0 flex-1 pt-[env(safe-area-inset-top)]">
        <RouterOutlet pathname={router.pathname} />
      </main>
      {#if !keyboard.open}<TabBar />{/if}
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
              {#if pageHeader.current?.heading}
                <div class="ml-2 flex min-w-0 items-center gap-2">{@render pageHeader.current?.heading?.()}</div>
              {:else if pageHeader.current}
                {@const Icon = pageHeader.current?.icon}
                {#if Icon}
                  <span
                    class="ml-2 shrink-0"
                    style={pageHeader.current?.hue ? `color: var(${pageHeader.current.hue})` : undefined}
                  >
                    <Icon class="size-[18px]" />
                  </span>
                {/if}
                <h1
                  class="ml-2 truncate font-display text-lg"
                  style="font-weight: var(--display-weight); letter-spacing: var(--display-track)"
                >
                  {pageHeader.current?.title}
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
  <ModuleOverlays enabled={enabled.list} />
  <Toaster />
</TooltipProvider>
