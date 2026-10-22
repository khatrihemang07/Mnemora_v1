import { describe, expect, it } from 'vitest';
import { BarGate, PROGRESS_BAR_DELAY_MS } from './bar-gate';

/** A timer the test advances by hand. */
function fakeTimers() {
  let now = 0;
  const pending = new Set<{ at: number; fn: () => void }>();
  return {
    setTimer: (fn: () => void, ms: number) => {
      const entry = { at: now + ms, fn };
      pending.add(entry);
      return () => pending.delete(entry);
    },
    advance(ms: number) {
      now += ms;
      for (const entry of [...pending]) {
        if (entry.at <= now) {
          pending.delete(entry);
          entry.fn();
        }
      }
    },
  };
}

function setup() {
  const timers = fakeTimers();
  const changes: boolean[] = [];
  const gate = new BarGate((v) => changes.push(v), timers.setTimer);
  return { timers, changes, gate };
}

describe('the progress bar gate (600 ms rule)', () => {
  it('waits 600 ms', () => {
    expect(PROGRESS_BAR_DELAY_MS).toBe(600);
  });

  it('stays hidden for a sync that ends sooner', () => {
    const { timers, changes, gate } = setup();
    gate.update(true);
    timers.advance(599);
    expect(gate.visible).toBe(false);
    gate.update(false);
    timers.advance(1_000);
    expect(gate.visible).toBe(false);
    expect(changes).toEqual([]);
  });

  it('appears once the sync has run for 600 ms', () => {
    const { timers, changes, gate } = setup();
    gate.update(true);
    timers.advance(600);
    expect(gate.visible).toBe(true);
    expect(changes).toEqual([true]);
  });

  it('goes away when the sync ends', () => {
    const { timers, changes, gate } = setup();
    gate.update(true);
    timers.advance(700);
    gate.update(false);
    expect(gate.visible).toBe(false);
    expect(changes).toEqual([true, false]);
  });

  it('progress updates do not restart the wait', () => {
    const { timers, gate } = setup();
    gate.update(true);
    timers.advance(400);
    gate.update(true);
    timers.advance(200);
    expect(gate.visible).toBe(true);
  });

  it('a second sync waits its own 600 ms', () => {
    const { timers, gate } = setup();
    gate.update(true);
    timers.advance(700);
    gate.update(false);
    gate.update(true);
    timers.advance(300);
    expect(gate.visible).toBe(false);
    timers.advance(300);
    expect(gate.visible).toBe(true);
  });

  it('stop cancels a pending reveal', () => {
    const { timers, gate } = setup();
    gate.update(true);
    gate.stop();
    timers.advance(1_000);
    expect(gate.visible).toBe(false);
  });
});
