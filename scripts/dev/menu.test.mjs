import { describe, expect, it } from 'vitest';
import { DEV_URL } from './config.mjs';
import { formatMenu, formatStatus } from './menu.mjs';

const idle = {
  environment: 'Production',
  states: { mac: 'stopped', android: 'stopped' },
  environments: { mac: 'Production', android: 'Sandbox' },
  queue: [],
  keysAvailable: true,
};

describe('formatMenu', () => {
  it('lists every key, the Environment of each native app, and Ctrl-C', () => {
    const text = formatMenu(idle);
    for (const line of [
      '  w        open the web mock',
      '  z        launch the Mac app (Production)',
      '  a        launch the Android app (Sandbox)',
      '  r        relaunch',
      '  x        stop the native app(s), keep Vite',
      '  ?        show this menu',
      '  q        quit everything',
      'Ctrl-C   quit everything',
      DEV_URL,
    ]) {
      expect(text).toContain(line);
    }
  });

  it('leaves out a target the session does not offer', () => {
    const text = formatMenu({ ...idle, states: { mac: 'stopped' } });
    expect(text).toContain('launch the Mac app (Production)');
    expect(text).not.toContain('Android');
  });

  it('shows each target state and the queue, read from the states', () => {
    const text = formatMenu({
      ...idle,
      states: { mac: 'running', android: 'preparing' },
      queue: ['android'],
    });
    expect(text).toContain('Mac: running');
    expect(text).toContain('Android: preparing');
    expect(text).toContain('Queue: Android');
  });

  it('says keys are unavailable when there is no terminal', () => {
    expect(formatMenu({ ...idle, keysAvailable: false })).toContain(
      'Keys unavailable (not a terminal)',
    );
  });
});

describe('formatStatus', () => {
  it('is one line with the Environment, each target state and the queue', () => {
    const line = formatStatus({
      environment: 'Sandbox',
      states: { mac: 'building', android: 'queued' },
      queue: ['android'],
    });
    expect(line).not.toContain('\n');
    expect(line).toBe('Mnemora dev · Sandbox · Mac: building · Android: queued · Queue: Android');
  });

  it('says the queue is empty as none', () => {
    const line = formatStatus({
      environment: 'Production',
      states: { mac: 'stopped', android: 'stopped' },
      queue: [],
    });
    expect(line).toMatch(/Queue: none$/);
  });
});
