import { describe, expect, it } from 'vitest';
import type { Pause } from '../../lib/ipc/bindings';
import { durationWords, moduleName, nameList, pauseBanner } from './pause';

const MIN = 60_000;
const HOUR = 60 * MIN;

const banner = (pause: Pause | null, name = 'Pixel 8') => pauseBanner({ name, pause });

describe('how far a clock is out', () => {
  it('counts minutes under an hour', () => {
    expect(durationWords(6 * MIN)).toBe('6 min');
    expect(durationWords(59 * MIN + 20_000)).toBe('59 min');
    expect(durationWords(5_000)).toBe('1 min');
  });

  it('counts hours, with the minutes while under ten hours', () => {
    expect(durationWords(HOUR)).toBe('1 h');
    expect(durationWords(3 * HOUR)).toBe('3 h');
    expect(durationWords(3 * HOUR + 20 * MIN)).toBe('3 h 20 min');
    expect(durationWords(3 * HOUR + 29_000)).toBe('3 h');
    expect(durationWords(10 * HOUR + 40 * MIN)).toBe('11 h');
    expect(durationWords(47 * HOUR)).toBe('47 h');
  });

  it('counts days from two days', () => {
    expect(durationWords(48 * HOUR)).toBe('2 d');
    expect(durationWords(400 * 24 * HOUR)).toBe('400 d');
  });
});

describe('Module names', () => {
  it('are the ones people know', () => {
    expect(moduleName('todo')).toBe('Todo');
    expect(moduleName('core')).toBe('Settings');
    expect(moduleName('habits')).toBe('Habits');
    expect(nameList(['todo'])).toBe('Todo');
    expect(nameList(['todo', 'journal'])).toBe('Todo and Journal');
    expect(nameList(['todo', 'journal', 'core'])).toBe('Todo, Journal and Settings');
  });
});

describe('the paused banner', () => {
  it('says nothing for a device that is not paused', () => {
    expect(banner(null)).toBeNull();
  });

  it('tells the other device to update when its Module is older', () => {
    expect(banner({ kind: 'update', older_here: [], older_there: ['todo'] })).toEqual({
      title: 'Paused with Pixel 8',
      text: 'Update Mnemora on Pixel 8 to keep syncing: it has an older Todo.',
    });
    expect(banner({ kind: 'update', older_here: [], older_there: ['todo', 'journal'] })?.text).toBe(
      'Update Mnemora on Pixel 8 to keep syncing: it has an older Todo and Journal.',
    );
  });

  it('tells this device to update when it is the older one', () => {
    expect(banner({ kind: 'update', older_here: ['journal'], older_there: [] })?.text).toBe(
      'Update Mnemora on this device to keep syncing: this device has an older Journal.',
    );
  });

  it('asks for both when each is older somewhere', () => {
    expect(banner({ kind: 'update', older_here: ['journal'], older_there: ['todo'] })?.text).toBe(
      'Update Mnemora on both devices to keep syncing: Pixel 8 has an older Todo and this device an older Journal.',
    );
  });

  it('says when records made on a wrong clock will be taken, though the clock is fixed now', () => {
    expect(banner({ kind: 'stamped', resumes_in_ms: 160 * MIN })?.text).toBe(
      'Pixel 8 has changes made while a clock was set ahead. Syncing resumes by itself in 2 h 40 min.',
    );
  });

  it('names a clock that is ahead, theirs or ours', () => {
    expect(banner({ kind: 'clock', ahead_ms: 3 * HOUR, here: false })?.text).toBe(
      'Pixel 8’s clock is 3 h ahead. Fix the time to keep syncing.',
    );
    expect(banner({ kind: 'clock', ahead_ms: 12 * MIN, here: true })?.text).toBe(
      'This device’s clock is 12 min ahead. Fix the time to keep syncing.',
    );
  });
});
