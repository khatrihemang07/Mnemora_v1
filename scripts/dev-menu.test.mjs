import { describe, expect, it } from 'vitest';
import {
  buildDoneMarker,
  createBuildQueue,
  DEV_PORT,
  DISABLE_BEFORE_DEV,
  descendantPids,
  formatMenu,
  keyToAction,
  nativeCommand,
  parseAdbDevices,
  parseArgs,
} from './dev-menu.mjs';

describe('parseArgs', () => {
  it('defaults to nothing launched and Production', () => {
    expect(parseArgs([])).toEqual({ sandbox: false, web: false, mac: false, android: false });
  });

  it('combines flags in any order', () => {
    expect(parseArgs(['--android', '--mac', '--web'])).toEqual({
      sandbox: false,
      web: true,
      mac: true,
      android: true,
    });
  });

  it('reads --sandbox as the Sandbox Environment for the Mac app', () => {
    expect(parseArgs(['--sandbox', '--mac']).sandbox).toBe(true);
  });

  it('rejects an unknown flag so a typo never silently does nothing', () => {
    expect(() => parseArgs(['--macc'])).toThrow(/Unknown option: --macc/);
  });
});

describe('keyToAction', () => {
  it('maps the menu keys', () => {
    expect(keyToAction('w')).toBe('web');
    expect(keyToAction('q')).toBe('mac');
    expect(keyToAction('a')).toBe('android');
    expect(keyToAction('r')).toBe('relaunch');
    expect(keyToAction('x')).toBe('stop');
    expect(keyToAction('?')).toBe('help');
  });

  it('maps Ctrl-C to quit (the terminal sends it as a character in raw mode)', () => {
    expect(keyToAction('\u0003')).toBe('quit');
  });

  it('ignores every other key', () => {
    expect(keyToAction('z')).toBeNull();
    expect(keyToAction('\r')).toBeNull();
    expect(keyToAction('')).toBeNull();
  });
});

describe('nativeCommand', () => {
  const disable = ['--config', DISABLE_BEFORE_DEV];

  it('disables beforeDevCommand so Tauri does not start a second Vite', () => {
    expect(DISABLE_BEFORE_DEV).toBe('{"build":{"beforeDevCommand":""}}');
    expect(JSON.parse(DISABLE_BEFORE_DEV).build.beforeDevCommand).toBe('');
  });

  it('runs Production on the Mac with the plain tauri dev command', () => {
    const cmd = nativeCommand('mac', { sandbox: false });
    expect(cmd.command).toBe('cargo');
    expect(cmd.args).toEqual(['tauri', 'dev', ...disable]);
    expect(cmd.env.CARGO_BUILD_JOBS).toBe('2');
    expect(cmd.env.MNEMORA_ANDROID_SANDBOX).toBeUndefined();
  });

  it('passes the sandbox config and feature on the Mac in Sandbox', () => {
    const cmd = nativeCommand('mac', { sandbox: true });
    expect(cmd.args).toEqual([
      'tauri',
      'dev',
      '--config',
      'src-tauri/tauri.sandbox.conf.json',
      '--features',
      'sandbox',
      ...disable,
    ]);
  });

  it('always uses Sandbox for Android, whatever the Mac Environment is', () => {
    for (const sandbox of [false, true]) {
      const cmd = nativeCommand('android', { sandbox });
      expect(cmd.command).toBe('cargo');
      expect(cmd.args).toEqual(['tauri', 'android', 'dev', '--features', 'sandbox', ...disable]);
      expect(cmd.env.MNEMORA_ANDROID_SANDBOX).toBe('1');
      expect(cmd.env.CARGO_BUILD_JOBS).toBe('2');
    }
  });
});

describe('parseAdbDevices', () => {
  it('returns the serials that are online', () => {
    const text = 'List of devices attached\n10BD8H0UM50004X\tdevice\nemulator-5554\toffline\n\n';
    expect(parseAdbDevices(text)).toEqual(['10BD8H0UM50004X']);
  });

  it('returns nothing when no phone is attached', () => {
    expect(parseAdbDevices('List of devices attached\n\n')).toEqual([]);
  });
});

describe('buildDoneMarker', () => {
  it('ends the Mac build phase when cargo finishes or the app starts', () => {
    expect(
      buildDoneMarker('mac', '    Finished `dev` profile [unoptimized] target(s) in 3.2s'),
    ).toBe(true);
    expect(buildDoneMarker('mac', '     Running `target/debug/mnemora-app`')).toBe(true);
    expect(buildDoneMarker('mac', '   Compiling mnemora-core v0.1.0')).toBe(false);
  });

  it('ends the Android build phase only after Gradle has finished', () => {
    expect(buildDoneMarker('android', 'BUILD SUCCESSFUL in 41s')).toBe(true);
    expect(buildDoneMarker('android', '    Finished `dev` profile [unoptimized] target(s)')).toBe(
      false,
    );
  });
});

describe('createBuildQueue', () => {
  // startJob records the target and hands back a release the test controls,
  // so the queue's one-at-a-time rule is observable without spawning anything.
  function harness() {
    const started = [];
    const queue = createBuildQueue((target, release) => {
      started.push({ target, release });
    });
    return { started, queue };
  }

  it('starts the first build at once and holds the rest', () => {
    const { started, queue } = harness();
    expect(queue.enqueue('mac')).toBe(true);
    expect(queue.enqueue('android')).toBe(true);
    expect(started.map((s) => s.target)).toEqual(['mac']);
    expect(queue.buildingTarget()).toBe('mac');
    expect(queue.pendingTargets()).toEqual(['android']);
  });

  it('starts the next build only after the current one releases its slot', () => {
    const { started, queue } = harness();
    queue.enqueue('mac');
    queue.enqueue('android');
    started[0].release();
    expect(started.map((s) => s.target)).toEqual(['mac', 'android']);
    expect(queue.pendingTargets()).toEqual([]);
  });

  it('ignores a second release of the same slot', () => {
    const { started, queue } = harness();
    queue.enqueue('mac');
    queue.enqueue('android');
    started[0].release();
    started[0].release();
    expect(started.map((s) => s.target)).toEqual(['mac', 'android']);
  });

  it('refuses a target that is already waiting', () => {
    const { queue } = harness();
    queue.enqueue('mac');
    queue.enqueue('android');
    expect(queue.enqueue('android')).toBe(false);
    expect(queue.pendingTargets()).toEqual(['android']);
  });

  it('cancels only the builds that have not started', () => {
    const { started, queue } = harness();
    queue.enqueue('mac');
    queue.enqueue('android');
    expect(queue.cancelPending()).toEqual(['android']);
    started[0].release();
    expect(started.map((s) => s.target)).toEqual(['mac']);
    expect(queue.buildingTarget()).toBeNull();
  });
});

describe('descendantPids', () => {
  it('finds every process under the root, deepest first, and not the root itself', () => {
    const ps = ['100 1', '200 100', '300 200', '301 200', '400 1', ''].join('\n');
    expect(descendantPids(ps, 100)).toEqual([300, 301, 200]);
  });

  it('is empty when the root has no children', () => {
    expect(descendantPids('100 1\n', 100)).toEqual([]);
  });
});

describe('formatMenu', () => {
  const base = {
    environment: 'Production',
    natives: {},
    building: null,
    pending: [],
    keysAvailable: true,
  };

  it('lists every key and Ctrl-C with the Environment named', () => {
    const text = formatMenu({ ...base, port: DEV_PORT });
    for (const line of [
      'w',
      'q',
      'a',
      'r',
      'x',
      '?',
      'Ctrl-C',
      `localhost:${DEV_PORT}`,
      'Production',
    ]) {
      expect(text).toContain(line);
    }
  });

  it('shows the native state and the queue', () => {
    const text = formatMenu({
      ...base,
      port: DEV_PORT,
      natives: { mac: 'running' },
      building: 'android',
      pending: ['mac'],
    });
    expect(text).toContain('Mac: running');
    expect(text).toContain('Android: building');
    expect(text).toContain('Queue: mac');
  });

  it('says keys are unavailable when there is no terminal', () => {
    const text = formatMenu({ ...base, port: DEV_PORT, keysAvailable: false });
    expect(text).toContain('Keys unavailable (not a terminal)');
  });
});
