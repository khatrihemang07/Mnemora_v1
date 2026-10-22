import { describe, expect, it } from 'vitest';
import { clock, fractionLeft, isBusy, problem, remaining } from './pair-view';

describe('problem', () => {
  it('words a wrong code with the tries left and where to look', () => {
    const p = problem({ kind: 'wrong_code', tries_left: 2, name: "Hemang's MacBook Pro" }, true);
    expect(p.title).toBe("That code didn't match.");
    expect(p.fix).toBe("2 tries left. Check the code on Hemang's MacBook Pro.");
    expect(p.deadCode).toBe(false);
  });

  it('says "1 try" in the singular and tells a dead code to be replaced', () => {
    expect(problem({ kind: 'wrong_code', tries_left: 1, name: 'Mac' }, true).fix).toBe(
      '1 try left. Check the code on Mac.',
    );
    const dead = problem({ kind: 'wrong_code', tries_left: 0, name: 'Mac' }, true);
    expect(dead.deadCode).toBe(true);
    expect(dead.fix).toContain('Press New code on Mac');
  });

  it('words an expired code differently on the two devices', () => {
    const typing = problem({ kind: 'code_expired', name: 'Mac' }, true);
    expect(typing.fix).toBe('Press New code on Mac and type the new one.');
    const showing = problem({ kind: 'code_expired', name: null }, false);
    expect(showing.title).toBe('This code expired.');
    expect(problem({ kind: 'code_expired', name: null }, true).fix).toContain('the other device');
  });

  it('names the device to update when versions differ', () => {
    const them = problem({ kind: 'version_mismatch', name: 'Pixel 8', older: 'them' }, true);
    expect(them.title).toBe('Update Mnemora on Pixel 8 first.');
    const me = problem({ kind: 'version_mismatch', name: 'Pixel 8', older: 'me' }, true);
    expect(me.title).toBe('Update Mnemora on this device first.');
    expect(problem({ kind: 'version_mismatch', name: 'Pixel 8', older: 'both' }, true).title).toBe(
      'Update Mnemora on both devices first.',
    );
  });

  it('has a fix for every failure', () => {
    const errors = [
      { kind: 'tries_used' },
      { kind: 'declined', name: 'Pixel 8' },
      { kind: 'cancelled', name: null },
      { kind: 'different_environment', theirs: 'sandbox' },
      { kind: 'different_groups' },
      { kind: 'nothing_nearby' },
      { kind: 'connection_lost' },
      { kind: 'busy' },
      { kind: 'not_ready' },
    ] as const;
    for (const e of errors) {
      const p = problem(e, true);
      expect(p.title.length, e.kind).toBeGreaterThan(0);
      expect(p.fix.length, e.kind).toBeGreaterThan(0);
    }
  });
});

describe('countdown', () => {
  it('shows minutes and seconds, rounding up', () => {
    expect(clock(114_000)).toBe('1:54');
    expect(clock(120_000)).toBe('2:00');
    expect(clock(59_001)).toBe('1:00');
    expect(clock(1)).toBe('0:01');
    expect(clock(0)).toBe('0:00');
    expect(clock(-5)).toBe('0:00');
  });

  it('keeps the ring between empty and full', () => {
    expect(fractionLeft(60_000, 120_000)).toBe(0.5);
    expect(fractionLeft(200_000, 120_000)).toBe(1);
    expect(fractionLeft(-1, 120_000)).toBe(0);
    expect(fractionLeft(5, 0)).toBe(0);
  });

  it('counts down from when the view arrived', () => {
    const view = {
      state: 'showing',
      code: '4821 0937',
      life_ms: 120_000,
      left_ms: 90_000,
      tries_left: 3,
      discoverable: true,
      notice: null,
    } as const;
    expect(remaining(view, 30_000)).toBe(60_000);
    expect(remaining(view, 500_000)).toBe(0);
    expect(remaining({ state: 'idle' }, 0)).toBe(0);
  });
});

describe('isBusy', () => {
  it('is false at rest and after an ending, true in between', () => {
    expect(isBusy({ state: 'idle' })).toBe(false);
    expect(isBusy({ state: 'failed', error: { kind: 'busy' } })).toBe(false);
    expect(isBusy({ state: 'searching' })).toBe(true);
    expect(isBusy({ state: 'waiting', name: 'Mac' })).toBe(true);
  });
});
