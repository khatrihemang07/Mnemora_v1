import { describe, expect, it } from 'vitest';
import { BUILD_SLOT_TIMEOUT_MS, DEV_URL } from './config.mjs';
import { createController } from './controller.mjs';
import { ACTION } from './keys.mjs';
import { fakeDeps, fakeTimers, flush } from './test-fakes.mjs';

const MAC_FINISHED = '    Finished `dev` profile [unoptimized] target(s) in 2.0s';
const ANDROID_INSTALL = "Installing APK 'app-arm64-debug.apk' on 'PHONE'";
const PHONE_READY = 'List of devices attached\nPHONE\tdevice\n';

function build({ sandbox = true, autoAdb = true } = {}) {
  const fake = fakeDeps({ autoAdb });
  const timers = fakeTimers();
  const logs = [];
  const controller = createController({
    sandbox,
    keysAvailable: true,
    deps: fake.deps,
    log: (message) => logs.push(message),
    timers,
  });
  const launch = (target) => controller.dispatch({ action: ACTION.LAUNCH, target });
  const relaunch = () => controller.dispatch({ action: ACTION.RELAUNCH });
  const stop = () => controller.dispatch({ action: ACTION.STOP });
  return { controller, ...fake, timers, logs, launch, relaunch, stop };
}

const statusLines = (logs) => logs.filter((line) => line.startsWith('Mnemora dev'));
const lastStatus = (logs) => statusLines(logs).at(-1);
const cargoBuilds = (spawns) => spawns.filter((spawn) => spawn.command === 'cargo');
const buildsOf = (spawns, id) =>
  spawns.filter((spawn) => spawn.args[1] === (id === 'mac' ? 'dev' : 'android'));

describe('launching', () => {
  it('builds the Mac app with the Production command when Sandbox is off', () => {
    const { launch, spawns, logs } = build({ sandbox: false });
    launch('mac');
    expect(spawns).toHaveLength(1);
    expect(spawns[0].command).toBe('cargo');
    expect(spawns[0].args).not.toContain('sandbox');
    expect(logs).toContain('Mac: building (Production)');
  });

  it('refuses Android in Production, builds nothing and points at make sandbox', () => {
    const { launch, spawns, logs } = build({ sandbox: false });
    launch('android');
    expect(spawns).toHaveLength(0);
    expect(logs).toContain('Android runs on Sandbox data only. Use make sandbox.');
  });

  it('passes the sandbox config and feature to the Mac build in Sandbox', () => {
    const { launch, spawns } = build({ sandbox: true });
    launch('mac');
    expect(spawns[0].args).toEqual(
      expect.arrayContaining([
        '--config',
        'src-tauri/tauri.sandbox.conf.json',
        '--features',
        'sandbox',
      ]),
    );
  });

  it('says a target that is already building is building, and does not start it again', () => {
    const { launch, spawns, logs } = build();
    launch('mac');
    launch('mac');
    expect(spawns).toHaveLength(1);
    expect(logs).toContain('Mac is building. r relaunches it.');
  });

  it('says a target that is already queued is queued', () => {
    const { launch, logs } = build();
    launch('mac');
    launch('android');
    launch('android');
    expect(logs).toContain('Android is already queued.');
  });

  it('queues a second launch behind the one building, and says so', () => {
    const { launch, spawns, logs } = build();
    launch('mac');
    launch('android');
    expect(spawns).toHaveLength(1);
    expect(logs).toContain('Android is queued behind the current native build.');
  });

  it('opens the web mock at the dev URL', () => {
    const { controller, opened } = build();
    controller.dispatch({ action: ACTION.WEB });
    expect(opened).toEqual([DEV_URL]);
  });
});

describe('the build slot', () => {
  it('frees the Mac slot at the cargo Finished line, so the running Mac app holds no slot', async () => {
    const { launch, spawns, logs } = build();
    launch('mac');
    launch('android');
    spawns[0].handlers.onLine(MAC_FINISHED);
    await flush();
    expect(logs.some((line) => line.includes('Mac: running'))).toBe(true);
    // Android got the slot at once and is now building.
    expect(buildsOf(spawns, 'android')).toHaveLength(1);
  });

  it('holds the Android slot past BUILD SUCCESSFUL, which Gradle can print more than once', async () => {
    const { launch, spawns } = build();
    launch('android');
    launch('mac');
    await flush();
    spawns[0].handlers.onLine('BUILD SUCCESSFUL in 41s');
    await flush();
    expect(buildsOf(spawns, 'mac')).toHaveLength(0);
    spawns[0].handlers.onLine(ANDROID_INSTALL);
    await flush();
    expect(buildsOf(spawns, 'mac')).toHaveLength(1);
  });

  it('frees the slot when the child exits without a marker', async () => {
    const { launch, spawns } = build();
    launch('mac');
    launch('android');
    spawns[0].handlers.onExit(1, null);
    await flush();
    expect(buildsOf(spawns, 'android')).toHaveLength(1);
  });

  it('frees the slot after the timeout when no marker ever shows, so a silent build cannot block the queue', async () => {
    const { launch, timers, spawns, logs } = build();
    launch('mac');
    launch('android');
    timers.advance(BUILD_SLOT_TIMEOUT_MS);
    await flush();
    expect(logs.some((line) => line.includes('no build marker'))).toBe(true);
    expect(buildsOf(spawns, 'android')).toHaveLength(1);
  });
});

describe('Android preparation', () => {
  it('reports unauthorized with a specific message and never says No phone found', async () => {
    const { launch, runs, spawns, logs } = build({ autoAdb: false });
    launch('android');
    runs[0].finish({ stdout: 'List of devices attached\nPHONE\tunauthorized\n' });
    await flush();
    const text = logs.join('\n');
    expect(text).toMatch(/not authorised for USB debugging/);
    expect(text).not.toMatch(/No phone found/);
    expect(spawns).toHaveLength(0);
  });

  it('reports offline with its own message', async () => {
    const { launch, runs, logs } = build({ autoAdb: false });
    launch('android');
    runs[0].finish({ stdout: 'List of devices attached\nPHONE\toffline\n' });
    await flush();
    expect(logs.join('\n')).toMatch(/offline to adb/);
  });

  it('does not start the build when adb reverse fails, says why, and undoes nothing', async () => {
    const { launch, runs, spawns, logs } = build({ autoAdb: false });
    launch('android');
    runs[0].finish({ stdout: PHONE_READY });
    await flush();
    runs[1].finish({ code: 1, stderr: 'adb: error: device offline\n' });
    await flush();
    expect(spawns).toHaveLength(0);
    expect(logs.join('\n')).toMatch(/adb reverse failed \(exit 1\): adb: error: device offline/);
    expect(runs.some((run) => run.args.includes('--remove'))).toBe(false);
    expect(lastStatus(logs)).toContain('Android: stopped');
  });

  it('builds Android in Sandbox, then removes the port forward when the build stops', async () => {
    const { launch, spawns, runs } = build();
    launch('android');
    await flush();
    expect(spawns[0].env.MNEMORA_ANDROID_SANDBOX).toBe('1');
    expect(runs.some((run) => run.args.includes('reverse'))).toBe(true);
    spawns[0].handlers.onExit(0, null);
    await flush();
    expect(runs.some((run) => run.args.includes('--remove'))).toBe(true);
  });
});

describe('relaunch (r)', () => {
  it('restarts a building Mac app and puts it back in the queue behind the waiting Android build', async () => {
    const { relaunch, launch, spawns, killed, logs } = build();
    launch('mac');
    launch('android');
    relaunch();
    expect(killed).toEqual([spawns[0].pid]);
    spawns[0].handlers.onExit(null, 'SIGTERM');
    await flush();
    expect(buildsOf(spawns, 'android')).toHaveLength(1);
    expect(lastStatus(logs)).toContain('Mac: queued');
  });

  it('cancels a queued target and enqueues it again at the back', async () => {
    const { relaunch, launch, spawns, logs } = build();
    launch('mac');
    launch('android');
    relaunch();
    // Mac comes back to the queue behind the Android build that was waiting.
    spawns[0].handlers.onExit(null, 'SIGTERM');
    await flush();
    expect(lastStatus(logs)).toContain('Android: building');
    expect(lastStatus(logs)).toContain('Queue: Mac');
  });

  it('cancels an Android build that is still preparing, then prepares it again', async () => {
    const { relaunch, launch, runs, spawns } = build({ autoAdb: false });
    launch('android');
    expect(runs).toHaveLength(1);
    relaunch();
    expect(runs[0].cancelled).toBe(true);
    runs[0].finish({ code: null });
    await flush();
    expect(runs).toHaveLength(2);
    expect(spawns).toHaveLength(0);
    runs[1].finish({ stdout: PHONE_READY });
    await flush();
    expect(runs).toHaveLength(3);
  });

  it('says there is nothing to relaunch when nothing is native', () => {
    const { relaunch, logs } = build();
    relaunch();
    expect(logs).toContain('Nothing is running to relaunch.');
  });
});

describe('stop (x)', () => {
  it('stops the building native app and leaves the queued build queued, and says so', () => {
    const { stop, launch, spawns, killed, logs } = build({ autoAdb: false });
    launch('mac');
    launch('android');
    stop();
    expect(killed).toEqual([spawns[0].pid]);
    expect(logs.join('\n')).toMatch(/Stopping Mac\. Still queued: Android/);
  });

  it('does not restart the app it stopped', async () => {
    const { stop, launch, spawns, killed, logs } = build();
    launch('mac');
    stop();
    spawns[0].handlers.onExit(null, 'SIGTERM');
    await flush();
    expect(killed).toHaveLength(1);
    expect(cargoBuilds(spawns)).toHaveLength(1);
    expect(lastStatus(logs)).toContain('Mac: stopped');
  });

  it('says so when nothing native is running', () => {
    const { stop, logs } = build();
    stop();
    expect(logs).toContain('Nothing native is running.');
  });
});

describe('quitting (Ctrl-C) during Android preparation', () => {
  it('never spawns cargo when adb answers after quit has begun', async () => {
    const { controller, launch, runs, spawns } = build({ autoAdb: false });
    launch('android');
    const quitting = controller.shutdown();
    expect(runs[0].cancelled).toBe(true);
    // adb answered before the kill took effect: the reverse must not start, nor the build.
    runs[0].finish({ stdout: PHONE_READY });
    await quitting;
    await flush();
    expect(runs).toHaveLength(1);
    expect(spawns).toHaveLength(0);
  });

  it('does not start a queued build after quit', async () => {
    const { controller, launch, spawns } = build();
    launch('mac');
    launch('android');
    await controller.shutdown();
    spawns[0].handlers.onExit(1, null);
    await flush();
    expect(spawns).toHaveLength(1);
  });

  it('kills the running native tree and removes the Android port forward on quit', async () => {
    const { controller, launch, spawns, killed, runs } = build();
    launch('android');
    await flush();
    spawns[0].handlers.onLine(ANDROID_INSTALL);
    await controller.shutdown();
    expect(killed).toEqual([spawns[0].pid]);
    expect(runs.some((run) => run.args.includes('--remove'))).toBe(true);
  });

  it('refuses launch and relaunch once quit has begun', async () => {
    const { controller, launch, relaunch, spawns } = build();
    await controller.shutdown();
    launch('mac');
    relaunch();
    expect(spawns).toHaveLength(0);
  });
});

describe('a build that fails to start', () => {
  it('frees the slot, says why, and starts the next build', async () => {
    const { launch, spawns, logs } = build();
    launch('mac');
    launch('android');
    spawns[0].handlers.onExit(null, null, 'spawn cargo ENOENT');
    await flush();
    expect(logs.join('\n')).toContain('Mac: stopped (spawn cargo ENOENT)');
    expect(buildsOf(spawns, 'android')).toHaveLength(1);
  });
});

describe('status line', () => {
  it('prints the state after each change, so the queue is never stale', () => {
    const { launch, logs } = build();
    launch('mac');
    expect(lastStatus(logs)).toBe(
      'Mnemora dev · Sandbox · Mac: building · Android: stopped · Queue: none',
    );
  });
});
