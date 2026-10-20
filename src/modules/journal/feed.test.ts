import { describe, expect, it } from 'vitest';
import type { Page } from '../../lib/ipc/bindings';
import { buildFeed } from './feed';

const page = (date: string, entry_count = 1): Page => ({
  date,
  entry_count,
  word_count: entry_count * 3,
  entries: [],
});
const kinds = (items: ReturnType<typeof buildFeed>) =>
  items.map((i) => (i.kind === 'page' ? i.page.date : `# ${i.label}`));

describe('buildFeed', () => {
  it('is empty on a first run, so the empty state can show', () => {
    expect(buildFeed([], '2026-10-20')).toEqual([]);
  });
  it('always has Today, empty when nothing is written on it', () => {
    const items = buildFeed([page('2026-10-19')], '2026-10-20');
    expect(kinds(items)).toEqual(['2026-10-20', '2026-10-19']);
    const first = items[0];
    expect(first?.kind === 'page' && first.page.entry_count).toBe(0);
  });
  it('does not duplicate a Today that has Entries', () => {
    expect(kinds(buildFeed([page('2026-10-20'), page('2026-10-19')], '2026-10-20'))).toEqual([
      '2026-10-20',
      '2026-10-19',
    ]);
  });
  it('slots Today below future Pages', () => {
    expect(kinds(buildFeed([page('2026-10-25'), page('2026-10-19')], '2026-10-20'))).toEqual([
      '2026-10-25',
      '2026-10-20',
      '2026-10-19',
    ]);
  });
  it('puts a month divider before the first Page of an older month, never at the top', () => {
    const items = buildFeed(
      [
        page('2026-10-20'),
        page('2026-10-02'),
        page('2026-09-30'),
        page('2026-09-01'),
        page('2026-08-15'),
      ],
      '2026-10-20',
    );
    expect(kinds(items)).toEqual([
      '2026-10-20',
      '2026-10-02',
      '# September 2026',
      '2026-09-30',
      '2026-09-01',
      '# August 2026',
      '2026-08-15',
    ]);
  });
  it('keys items uniquely', () => {
    const items = buildFeed([page('2026-10-20'), page('2025-01-01')], '2026-10-20');
    expect(new Set(items.map((i) => i.key)).size).toBe(items.length);
  });
});
