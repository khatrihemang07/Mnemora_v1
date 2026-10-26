import { QueryClient } from '@tanstack/svelte-query';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

const handlers = vi.hoisted(() => new Map<string, () => void>());
const failing = vi.hoisted(() => new Set<string>());
vi.mock('./bindings', () => {
  const event = (name: string) => ({
    listen: async (handler: () => void) => {
      if (failing.has(name)) throw new Error(`cannot listen to ${name}`);
      handlers.set(name, handler);
      return () => handlers.delete(name);
    },
  });
  return {
    events: {
      settingsChangedEvent: event('settings'),
      todoChangedEvent: event('todo'),
      journalChangedEvent: event('journal'),
      aiChangedEvent: event('ai'),
    },
  };
});

import { startEventBridge } from './event-bridge';

/** A client with one fresh Todo query, one Journal query and a spy on what refetches. */
function setup() {
  const client = new QueryClient();
  const fetched: string[] = [];
  const observe = (key: string[], name: string) =>
    client.getQueryCache().build(client, {
      queryKey: key,
      queryFn: async () => {
        fetched.push(name);
        return name;
      },
    });
  const todo = observe(['todo', 'today', '2026-10-21'], 'todo');
  const journal = observe(['journal', 'pages'], 'journal');
  // Mark them active so invalidation refetches at once, as on-screen queries do.
  for (const query of [todo, journal]) {
    query.addObserver({
      options: query.options,
      onQueryUpdate: () => {},
      shouldFetchOnReconnect: () => false,
      shouldFetchOnWindowFocus: () => false,
      isActive: () => true,
      getCurrentQuery: () => query,
      setOptions: () => {},
    } as never);
  }
  return { client, fetched };
}

/** Tests run without a DOM: a stand-in document that can be hidden and shown. */
const fakeDocument = Object.assign(new EventTarget(), { hidden: false });

function setHidden(hidden: boolean) {
  fakeDocument.hidden = hidden;
  fakeDocument.dispatchEvent(new Event('visibilitychange'));
}

describe('startEventBridge, the Todo widget', () => {
  let stop: () => void;
  beforeEach(() => {
    handlers.clear();
    failing.clear();
    vi.stubGlobal('document', fakeDocument);
  });
  afterEach(() => {
    stop?.();
    setHidden(false);
    vi.unstubAllGlobals();
  });

  it('refetches only Todo when the Todo widget changed something', async () => {
    const { client, fetched } = setup();
    stop = await startEventBridge(client);
    handlers.get('todo')?.();
    await vi.waitFor(() => expect(fetched).toEqual(['todo']));
  });

  it('refetches Todo when the app becomes visible again, not when it is hidden', async () => {
    const { client, fetched } = setup();
    stop = await startEventBridge(client);
    setHidden(true);
    await Promise.resolve();
    expect(fetched).toEqual([]);
    setHidden(false);
    await vi.waitFor(() => expect(fetched).toEqual(['todo']));
  });

  it('stops listening when stopped', async () => {
    const { client, fetched } = setup();
    stop = await startEventBridge(client);
    stop();
    expect(handlers.size).toBe(0);
    setHidden(false);
    await Promise.resolve();
    expect(fetched).toEqual([]);
  });

  it('a Settings change still refreshes everything', async () => {
    const { client, fetched } = setup();
    stop = await startEventBridge(client);
    handlers.get('settings')?.();
    await vi.waitFor(() => expect(fetched.sort()).toEqual(['journal', 'todo']));
  });

  it('refetches only the Journal when Sync changed Journal data', async () => {
    const { client, fetched } = setup();
    stop = await startEventBridge(client);
    handlers.get('journal')?.();
    await vi.waitFor(() => expect(fetched).toEqual(['journal']));
  });

  it('refreshes AI queries when Sync changed AI data, and not the others', async () => {
    const { client, fetched } = setup();
    const ai = client.getQueryCache().build(client, {
      queryKey: ['ai', 'conversations'],
      queryFn: async () => {
        fetched.push('ai');
        return 'ai';
      },
    });
    ai.addObserver({
      options: ai.options,
      onQueryUpdate: () => {},
      shouldFetchOnReconnect: () => false,
      shouldFetchOnWindowFocus: () => false,
      isActive: () => true,
      getCurrentQuery: () => ai,
      setOptions: () => {},
    } as never);
    stop = await startEventBridge(client);
    handlers.get('ai')?.();
    await vi.waitFor(() => expect(fetched).toEqual(['ai']));
  });

  it('removes the earlier listeners when the Journal one cannot be registered', async () => {
    const { client } = setup();
    failing.add('journal');
    await expect(startEventBridge(client)).rejects.toThrow('cannot listen to journal');
    expect(handlers.size).toBe(0);
  });

  it('removes the first listener when the second cannot be registered', async () => {
    const { client } = setup();
    failing.add('todo');
    await expect(startEventBridge(client)).rejects.toThrow('cannot listen to todo');
    expect(handlers.size).toBe(0);
  });
});
