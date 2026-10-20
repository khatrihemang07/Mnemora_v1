import { describe, expect, it } from 'vitest';
import { LongPress } from './long-press';

function setup() {
  const timers: { fn: () => void; ms: number; live: boolean }[] = [];
  const presses: [number, number][] = [];
  const lp = new LongPress({
    onpress: (x, y) => presses.push([x, y]),
    setTimer: (fn, ms) => {
      const t = { fn, ms, live: true };
      timers.push(t);
      return () => {
        t.live = false;
      };
    },
  });
  const fire = () => {
    for (const t of timers) if (t.live) t.fn();
  };
  return { lp, presses, timers, fire };
}

describe('LongPress', () => {
  it('fires once, at the start position, after the delay', () => {
    const { lp, presses, timers, fire } = setup();
    lp.start(10, 20);
    expect(timers[0]?.ms).toBe(500);
    expect(presses).toEqual([]);
    fire();
    expect(presses).toEqual([[10, 20]]);
    expect(lp.fired).toBe(true);
  });

  it('is cancelled by lifting or by drifting past the slop', () => {
    const a = setup();
    a.lp.start(0, 0);
    a.lp.cancel();
    a.fire();
    expect(a.presses).toEqual([]);

    const b = setup();
    b.lp.start(0, 0);
    b.lp.move(4, 4);
    b.fire();
    expect(b.presses.length, 'small jitter still presses').toBe(1);

    const c = setup();
    c.lp.start(0, 0);
    c.lp.move(30, 0);
    c.fire();
    expect(c.presses).toEqual([]);
    expect(c.lp.fired).toBe(false);
  });

  it('a new press clears the swallow-the-click flag', () => {
    const { lp, fire } = setup();
    lp.start(0, 0);
    fire();
    lp.start(0, 0);
    expect(lp.fired).toBe(false);
  });
});
