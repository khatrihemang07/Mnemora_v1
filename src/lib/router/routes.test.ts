import { describe, expect, it } from 'vitest';
import type { ModuleManifest } from './manifest';
import { buildRoutes, navEntries } from './routes';

const comp = async () => ({ default: (() => {}) as never });
const manifest = (id: string, order: number, paths: `/${string}`[]): ModuleManifest => ({
  id,
  name: id,
  icon: {} as never,
  accentVar: `--color-${id}`,
  routes: paths.map((path) => ({ path, component: comp })),
  nav: { order },
});

describe('buildRoutes', () => {
  it('flattens every Module route into one map', () => {
    const routes = buildRoutes([
      manifest('home', 1, ['/']),
      manifest('settings', 9, ['/settings']),
    ]);
    expect(Object.keys(routes).sort()).toEqual(['/', '/settings']);
    expect(routes['/']).toBe(comp);
  });
  it('rejects two Modules claiming one path', () => {
    expect(() => buildRoutes([manifest('a', 1, ['/x']), manifest('b', 2, ['/x'])])).toThrow(/\/x/);
  });
});

describe('navEntries', () => {
  it('sorts by nav order and links to the first route', () => {
    const out = navEntries([manifest('settings', 9, ['/settings']), manifest('home', 1, ['/'])]);
    expect(out.map((e) => [e.id, e.path])).toEqual([
      ['home', '/'],
      ['settings', '/settings'],
    ]);
  });
});
