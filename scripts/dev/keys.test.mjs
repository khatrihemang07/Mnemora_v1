import { describe, expect, it } from 'vitest';
import { ACTION, keyToAction } from './keys.mjs';
import { TARGET_LIST } from './targets.mjs';

describe('keyToAction', () => {
  it('maps the core keys', () => {
    expect(keyToAction('w')).toEqual({ action: ACTION.WEB });
    expect(keyToAction('r')).toEqual({ action: ACTION.RELAUNCH });
    expect(keyToAction('x')).toEqual({ action: ACTION.STOP });
    expect(keyToAction('?')).toEqual({ action: ACTION.HELP });
  });

  it('maps each target key to a launch of that target', () => {
    expect(keyToAction('q')).toEqual({ action: ACTION.LAUNCH, target: 'mac' });
    expect(keyToAction('a')).toEqual({ action: ACTION.LAUNCH, target: 'android' });
  });

  it('maps Ctrl-C to quit (the terminal sends it as a character in raw mode)', () => {
    expect(keyToAction('\u0003')).toEqual({ action: ACTION.QUIT });
  });

  it('ignores every other key', () => {
    expect(keyToAction('z')).toBeNull();
    expect(keyToAction('\r')).toBeNull();
    expect(keyToAction('')).toBeNull();
  });

  it('gives every key one meaning, so a new target cannot shadow a core key', () => {
    const keys = ['w', 'r', 'x', '?', '\u0003', ...TARGET_LIST.map((target) => target.key)];
    expect(new Set(keys).size).toBe(keys.length);
  });
});
