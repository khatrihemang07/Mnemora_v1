import { describe, expect, it } from 'vitest';
import type { ModuleContext, RoutePath } from '../../lib/router';
import { bareKeyFor, enabledModules } from '../../shell/enabled-modules';
import { aiManifest } from './manifest';

/**
 * The bare keys the other Modules claim, and the page each is tied to (or none, for a key that
 * works everywhere). Kept in step with their manifests: the registry itself needs the router, so
 * the test cannot import them.
 */
const OTHERS: { module: string; key: string; page: RoutePath | null }[] = [
  { module: 'home', key: 'h', page: null },
  { module: 'journal', key: 'n', page: null },
  { module: 'journal', key: 'j', page: null },
  { module: 'journal', key: '/', page: '/journal' },
  { module: 'todo', key: 'q', page: null },
  { module: 'todo', key: 't', page: null },
  { module: 'todo', key: 'i', page: null },
  { module: 'todo', key: '/', page: '/todo' },
  { module: 'sync', key: 's', page: null },
  { module: 'settings', key: '?', page: null },
];

/** A context that is on one path. */
const on = (path: RoutePath): ModuleContext => ({
  navigate: () => {},
  isActive: (p) => p === path,
});

const key = (k: string, path: RoutePath) =>
  bareKeyFor(enabledModules([aiManifest], [], ['ai']), k, on(path));

describe('the AI Module keys', () => {
  it('A opens AI from anywhere but Todo, where G then A is the All view', () => {
    expect(key('a', '/journal')).not.toBeNull();
    expect(key('a', '/')).not.toBeNull();
    expect(key('a', '/todo')).toBeNull();
  });

  it('C and / are AI keys only on the AI page, so they keep their Journal and search meanings', () => {
    expect(key('c', '/ai')).not.toBeNull();
    expect(key('c', '/journal')).toBeNull();
    expect(key('/', '/ai')).not.toBeNull();
    expect(key('/', '/journal')).toBeNull();
  });

  it('none of the keys the AI Module claims is a key another Module uses on the AI page', () => {
    for (const claimed of ['a', 'c', '/']) {
      const rivals = OTHERS.filter((o) => o.key === claimed);
      for (const rival of rivals) {
        expect(rival.page, `${rival.module} ${rival.key}`).not.toBe('/ai');
        expect(rival.page === null, `${rival.module} ${rival.key} works everywhere`).toBe(false);
      }
    }
  });
});
