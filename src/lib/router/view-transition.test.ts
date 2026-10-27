import { afterEach, describe, expect, it, vi } from 'vitest';
import { withViewTransition } from './view-transition';

type Deferred = { promise: Promise<void>; resolve: () => void; reject: (error: unknown) => void };

function deferred(): Deferred {
  let resolve!: () => void;
  let reject!: (error: unknown) => void;
  const promise = new Promise<void>((res, rej) => {
    resolve = res;
    reject = rej;
  });
  return { promise, resolve, reject };
}

/** A document whose View Transitions the test settles by hand; the update callback runs at once. */
function fakeDocument() {
  const root = { dataset: {} as Record<string, string> };
  const transitions: Deferred[] = [];
  vi.stubGlobal('window', { matchMedia: () => ({ matches: false }) });
  vi.stubGlobal('document', {
    documentElement: root,
    startViewTransition: (update: () => Promise<void>) => {
      void update();
      const transition = deferred();
      transitions.push(transition);
      return { finished: transition.promise };
    },
  });
  const started = (index: number): Deferred => {
    const transition = transitions[index];
    if (!transition) throw new Error(`no transition ${index}`);
    return transition;
  };
  return { root, started };
}

afterEach(() => {
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
});

describe('route changes inside a View Transition', () => {
  it('a transition superseded by a newer one still completes the route change, without an error', async () => {
    const { started } = fakeDocument();
    const errors = vi.spyOn(console, 'error').mockImplementation(() => {});
    const first = withViewTransition(async () => 'first page');
    started(0).reject(
      new DOMException('Old view transition aborted by new view transition', 'AbortError'),
    );
    await expect(first).resolves.toBe('first page');
    expect(errors).not.toHaveBeenCalled();
  });

  it('a real error from the route change still reaches the caller', async () => {
    const { started } = fakeDocument();
    const failed = withViewTransition(async () => 'unused');
    started(0).reject(new Error('route failed'));
    await expect(failed).rejects.toThrow('route failed');
  });

  it('the fade-off marker stays on while a newer transition is still running', async () => {
    const { root, started } = fakeDocument();
    const first = withViewTransition(async () => 'a');
    const second = withViewTransition(async () => 'b');
    started(0).resolve();
    await expect(first).resolves.toBe('a');
    expect(root.dataset.viewTransition).toBe('');
    started(1).resolve();
    await expect(second).resolves.toBe('b');
    expect(root.dataset.viewTransition).toBeUndefined();
  });
});
