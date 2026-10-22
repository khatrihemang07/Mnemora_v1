import { describe, expect, it, vi } from 'vitest';

vi.mock('../lib/ipc/bindings', () => ({ commands: {}, events: {} }));
vi.mock('./registry', () => ({ modules: [] }));
vi.mock('./router', () => ({ router: {} }));

import type { ModuleManifest, RoutePath } from '../lib/router';
import { routeForLink, startLinkBridge } from './deep-links';

const seen: string[] = [];
const manifests = [
  {
    id: 'todo',
    openLink: (url: URL) => {
      seen.push(url.href);
      return '/todo?task=T1' as RoutePath;
    },
  },
  { id: 'journal' },
] as unknown as ModuleManifest[];

describe('routeForLink', () => {
  it("asks the host's Module where to go", () => {
    expect(routeForLink('mnemora://todo?task=T1', manifests)).toBe('/todo?task=T1');
    expect(seen).toContain('mnemora://todo?task=T1');
  });

  it.each([
    ['an unknown host', 'mnemora://nope?x=1'],
    ['a Module without the hook', 'mnemora://journal'],
    ['another scheme', 'https://todo/?task=T1'],
    ['text that is not a URL', 'not a url'],
    ['an empty link', ''],
  ])('goes Home for %s', (_, link) => {
    expect(routeForLink(link, manifests)).toBe('/');
  });
});

describe('startLinkBridge', () => {
  function bridge(queued: string[][]) {
    let announce = () => {};
    const navigated: string[] = [];
    const replaced: boolean[] = [];
    const start = startLinkBridge({
      take: async () => queued.shift() ?? [],
      onOpened: async (handler) => {
        announce = handler;
        return () => {};
      },
      navigate: (path, replace) => {
        navigated.push(path);
        replaced.push(replace);
      },
      manifests,
    });
    return { start, navigated, replaced, announce: () => announce() };
  }

  it('follows the link that was waiting when the app started', async () => {
    const b = bridge([['mnemora://todo?task=T1']]);
    await b.start;
    expect(b.navigated).toEqual(['/todo?task=T1']);
  });

  it('replaces Home for the link the app started with, and pushes the ones that come later', async () => {
    const b = bridge([['mnemora://todo?task=T1'], ['mnemora://todo?task=T2']]);
    await b.start;
    b.announce();
    await vi.waitFor(() => expect(b.navigated).toHaveLength(2));
    expect(b.replaced).toEqual([true, false]);
  });

  it('is done only once the first link has been followed', async () => {
    const b = bridge([['mnemora://todo?task=T1']]);
    await b.start;
    expect(b.navigated).toEqual(['/todo?task=T1']);
  });

  it('does nothing when no link is waiting', async () => {
    const b = bridge([]);
    await b.start;
    expect(b.navigated).toEqual([]);
  });

  it('follows a link announced while the app runs, and only the newest of several', async () => {
    const b = bridge([[], ['mnemora://nope', 'mnemora://todo?task=T1']]);
    await b.start;
    b.announce();
    await vi.waitFor(() => expect(b.navigated).toEqual(['/todo?task=T1']));
  });
});
