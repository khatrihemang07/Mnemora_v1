import { describe, expect, it } from 'vitest';
import type { PaletteItem } from './CommandPalette.svelte';
import { visibleItems } from './palette-filter';

const item = (id: string, label: string, extra: Partial<PaletteItem> = {}): PaletteItem => ({
  id,
  label,
  group: 'g',
  run: () => {},
  ...extra,
});

const items = [
  item('go', 'Go to Journal'),
  item('theme', 'Switch to dark theme'),
  item('find', 'Search Journal'),
  item('find-for', 'Search Journal', { queryLabel: (q) => `Search Journal for “${q}”` }),
];

const labels = (typed: string) => visibleItems(items, typed).map((v) => v.label);

describe('visibleItems', () => {
  it('shows every plain item and no query item when nothing is typed', () => {
    expect(labels('')).toEqual(['Go to Journal', 'Switch to dark theme', 'Search Journal']);
    expect(labels('   ')).toHaveLength(3);
  });

  it('keeps plain items whose label holds every typed word, in any case', () => {
    expect(labels('JOURNAL go')).toContain('Go to Journal');
    expect(labels('dark')).toContain('Switch to dark theme');
    expect(labels('dark')).not.toContain('Go to Journal');
  });

  it('adds the query item, worded with the trimmed text, after the plain ones', () => {
    expect(labels('  river ')).toEqual(['Search Journal for “river”']);
    const found = labels('journal');
    expect(found.at(-1)).toBe('Search Journal for “journal”');
    expect(found).toHaveLength(3);
  });
});
