<!--
  TabBar (phone): up to 4 tabs from core.phone_tabs, plus More for Settings and the rest. A Module can
  give its tab two extra gestures (`phoneTab`): tapping it while already there, and a long-press.
-->
<script lang="ts">
  import { createQuery } from '@tanstack/svelte-query';
  import Ellipsis from '~icons/lucide/ellipsis';
  import { navEntries, type PhoneTabActions } from '../lib/router';
  import { LongPress } from '../lib/long-press';
  import { readCore } from '../lib/settings/core';
  import { settingsQuery } from '../lib/settings/queries';
  import Sheet from '../lib/ui/Sheet.svelte';
  import ModuleBadge from './ModuleBadge.svelte';
  import { activeModuleId, isMoreActive, visibleTabs } from './phone-tabs';
  import { useEnabledModules } from './enabled.svelte';
  import { moduleContext } from './module-context';
  import { router } from './router';

  const settings = createQuery(() => settingsQuery);
  const enabled = useEnabledModules();
  const entries = $derived(navEntries(enabled.list));
  const ids = $derived(entries.map((e) => e.id));

  const tabIds = $derived(visibleTabs(readCore(settings.data?.values).phoneTabs, ids));
  const tabs = $derived(tabIds.flatMap((id) => entries.filter((e) => e.id === id)));
  const more = $derived(entries.filter((e) => !tabIds.includes(e.id)));
  const moreActive = $derived(isMoreActive(activeModuleId(router.pathname, entries), tabIds));
  let moreOpen = $state(false);

  // A long-press on a tab runs the Module's action; the tap that ends it must not also navigate.
  let held = false;
  let pressing: PhoneTabActions | undefined;
  const press = new LongPress({
    onpress: () => {
      held = true;
      pressing?.onLongPress?.(moduleContext);
    },
  });
  const touchLike = (e: PointerEvent) => e.pointerType !== 'mouse';

  function onTabClick(e: MouseEvent, tab: PhoneTabActions | undefined, active: boolean) {
    if (held) {
      held = false;
      e.preventDefault();
      return;
    }
    if (active && tab?.onActiveTap) {
      e.preventDefault();
      tab.onActiveTap(moduleContext);
    }
  }

  const item =
    'flex min-h-14 flex-1 cursor-pointer flex-col items-center justify-center gap-0.5 text-2xs transition-colors duration-(--dur) ease-out active:scale-[0.96]';
</script>

<nav
  aria-label="Main"
  class="flex shrink-0 border-t border-line bg-side pb-[env(safe-area-inset-bottom)]"
>
  {#each tabs as e (e.id)}
    {@const active = router.isActive(e.path)}
    <a
      href={router.href(e.path)}
      aria-current={active ? 'page' : undefined}
      class="{item} {active ? 'font-semibold text-ink' : 'text-muted'} [-webkit-touch-callout:none]"
      onclick={(ev) => onTabClick(ev, e.manifest.phoneTab, active)}
      onpointerdown={(ev) => {
        held = false;
        if (!e.manifest.phoneTab?.onLongPress || !touchLike(ev)) return;
        pressing = e.manifest.phoneTab;
        press.start(ev.clientX, ev.clientY);
      }}
      onpointermove={(ev) => press.move(ev.clientX, ev.clientY)}
      onpointerup={() => press.cancel()}
      onpointercancel={() => press.cancel()}
      oncontextmenu={(ev) => e.manifest.phoneTab?.onLongPress && ev.preventDefault()}
      draggable="false"
    >
      <span class="relative" style:color={active ? `var(${e.manifest.accentVar})` : 'inherit'}>
        <e.manifest.icon class="size-5" />
        {#if e.manifest.useBadge && e.manifest.phoneBadge !== false}<ModuleBadge
            manifest={e.manifest}
            placement="corner"
          />{/if}
      </span>
      {e.manifest.name}
    </a>
  {/each}
  <button
    type="button"
    aria-current={moreActive ? 'page' : undefined}
    class="{item} {moreActive ? 'font-semibold text-ink' : 'text-muted'}"
    onclick={() => (moreOpen = true)}
  >
    <span style:color={moreActive ? 'var(--accent)' : 'inherit'}><Ellipsis class="size-5" /></span>
    More
  </button>
</nav>

<Sheet open={moreOpen} title="More" onclose={() => (moreOpen = false)}>
  <ul>
    {#each more as e (e.id)}
      <li>
        <a
          href={router.href(e.path)}
          onclick={() => (moreOpen = false)}
          class="flex h-12 items-center gap-3 rounded-sm px-3 text-ink hover:bg-hover"
        >
          <span style:color="var({e.manifest.accentVar})"><e.manifest.icon class="size-5" /></span>
          {e.manifest.name}
        </a>
      </li>
    {/each}
  </ul>
</Sheet>
