export const MAX_TABS = 4;

/** Add or remove a tab; adding past the maximum returns the same array. */
export function toggleTab(tabs: string[], id: string): string[] {
  if (tabs.includes(id)) return tabs.filter((t) => t !== id);
  return tabs.length >= MAX_TABS ? tabs : [...tabs, id];
}

export function moveTab(tabs: string[], id: string, delta: -1 | 1): string[] {
  const from = tabs.indexOf(id);
  const to = from + delta;
  if (from < 0 || to < 0 || to >= tabs.length) return tabs;
  const next = [...tabs];
  next.splice(from, 1);
  next.splice(to, 0, id);
  return next;
}

/** Stored ids may name Modules that no longer exist. */
export const visibleTabs = (stored: string[], known: string[]) =>
  stored.filter((id) => known.includes(id)).slice(0, MAX_TABS);

/** The Module owning a pathname: exact match, or the longest path that prefixes it. */
export function activeModuleId(
  pathname: string,
  entries: { id: string; path: string }[],
): string | undefined {
  let best: { id: string; len: number } | undefined;
  for (const e of entries) {
    const hit = pathname === e.path || (e.path !== '/' && pathname.startsWith(`${e.path}/`));
    if (hit && (!best || e.path.length > best.len)) best = { id: e.id, len: e.path.length };
  }
  return best?.id;
}

/** "More" lights up when the current Module (Settings included) isn't pinned to a tab. */
export const isMoreActive = (activeId: string | undefined, tabIds: string[]) =>
  activeId !== undefined && !tabIds.includes(activeId);
