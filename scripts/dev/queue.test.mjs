import { describe, expect, it } from 'vitest';
import { createBuildQueue } from './queue.mjs';

// `start` records each job and hands back its release, which the test calls
// when a build phase ends, so the one-at-a-time rule is observable without processes.
function harness() {
  const started = [];
  const queue = createBuildQueue((slot) => started.push(slot));
  return { started, queue };
}

describe('createBuildQueue', () => {
  it('starts the first build at once and holds the rest', () => {
    const { started, queue } = harness();
    expect(queue.enqueue('mac')).toBe(true);
    expect(queue.enqueue('android')).toBe(true);
    expect(started.map((slot) => slot.target)).toEqual(['mac']);
    expect(queue.pendingTargets()).toEqual(['android']);
  });

  it('starts the next build only after the current one releases its slot', () => {
    const { started, queue } = harness();
    queue.enqueue('mac');
    queue.enqueue('android');
    started[0].release();
    expect(started.map((slot) => slot.target)).toEqual(['mac', 'android']);
    expect(queue.pendingTargets()).toEqual([]);
  });

  it('ignores a second release of the same slot, so it cannot free the next build twice', () => {
    const { started, queue } = harness();
    queue.enqueue('mac');
    queue.enqueue('android');
    queue.enqueue('mac');
    started[0].release();
    started[0].release();
    expect(started.map((slot) => slot.target)).toEqual(['mac', 'android']);
  });

  it('refuses a target that is already waiting', () => {
    const { queue } = harness();
    queue.enqueue('mac');
    queue.enqueue('android');
    expect(queue.enqueue('android')).toBe(false);
    expect(queue.pendingTargets()).toEqual(['android']);
  });

  it('removes one waiting target and leaves the others in order', () => {
    const { queue } = harness();
    queue.enqueue('mac');
    queue.enqueue('android');
    expect(queue.remove('android')).toBe(true);
    expect(queue.remove('android')).toBe(false);
    expect(queue.pendingTargets()).toEqual([]);
  });

  it('cancels only the builds that have not started', () => {
    const { started, queue } = harness();
    queue.enqueue('mac');
    queue.enqueue('android');
    expect(queue.cancelPending()).toEqual(['android']);
    started[0].release();
    expect(started.map((slot) => slot.target)).toEqual(['mac']);
  });

  it('starts a restarted target at the back of the queue', () => {
    const { started, queue } = harness();
    queue.enqueue('mac');
    queue.enqueue('android');
    queue.remove('android');
    queue.enqueue('android');
    expect(queue.pendingTargets()).toEqual(['android']);
    started[0].release();
    expect(started.map((slot) => slot.target)).toEqual(['mac', 'android']);
  });
});
