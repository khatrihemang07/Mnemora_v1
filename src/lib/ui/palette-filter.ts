import type { PaletteItem } from './CommandPalette.svelte';

/** An item as the palette shows it. */
export interface VisibleItem {
  item: PaletteItem;
  label: string;
}

/**
 * The items for what is typed. Plain items stay when every typed word appears in their label
 * (case ignored); with nothing typed they all show. Items with a `queryLabel` ("Search Journal
 * for …") show only once something is typed, worded with it, after the plain ones.
 */
export function visibleItems(items: PaletteItem[], typed: string): VisibleItem[] {
  const text = typed.trim();
  const words = text.toLowerCase().split(/\s+/).filter(Boolean);
  const plain = items
    .filter((i) => !i.queryLabel)
    .filter((i) => words.every((w) => i.label.toLowerCase().includes(w)))
    .map((item) => ({ item, label: item.label }));
  if (!text) return plain;
  const asking = items.flatMap((item) =>
    item.queryLabel ? [{ item, label: item.queryLabel(text) }] : [],
  );
  return [...plain, ...asking];
}
