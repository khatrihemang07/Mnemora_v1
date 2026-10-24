import { describe, expect, it } from 'vitest';
import { androidPrepare } from './android.mjs';
import { DEV_PORT } from './config.mjs';

// Fake `run`: answers each adb call from a script, and records the calls.
function fakeRun(answers) {
  const calls = [];
  const run = async (command, args) => {
    calls.push([command, ...args]);
    const answer = answers.shift();
    if (answer instanceof Error) throw answer;
    return { code: 0, stdout: '', stderr: '', ...answer };
  };
  return { run, calls };
}

const PHONE_LIST = { stdout: 'List of devices attached\nPHONE\tdevice\n' };

describe('androidPrepare', () => {
  it('forwards the dev port to the phone and returns the undo for that forward', async () => {
    const { run, calls } = fakeRun([PHONE_LIST, {}]);
    const undo = await androidPrepare(run);
    expect(calls).toEqual([
      ['adb', 'devices'],
      ['adb', '-s', 'PHONE', 'reverse', `tcp:${DEV_PORT}`, `tcp:${DEV_PORT}`],
    ]);
    expect(undo).toEqual({
      command: 'adb',
      args: ['-s', 'PHONE', 'reverse', '--remove', `tcp:${DEV_PORT}`],
    });
  });

  it('fails with the reverse exit code and stderr when adb reverse fails, and returns no undo', async () => {
    const { run } = fakeRun([
      PHONE_LIST,
      { code: 1, stderr: 'adb: error: failed to reverse: device offline\n' },
    ]);
    await expect(androidPrepare(run)).rejects.toThrow(
      /adb reverse failed \(exit 1\): adb: error: failed to reverse/,
    );
  });

  it('says adb devices itself failed when its exit code is not zero', async () => {
    const { run } = fakeRun([{ code: 1, stderr: 'boom' }]);
    await expect(androidPrepare(run)).rejects.toThrow(/adb devices failed \(exit 1\)/);
  });

  it('reports a missing adb as not found, whatever the spawn error was', async () => {
    const { run } = fakeRun([new Error('spawn adb ENOENT')]);
    await expect(androidPrepare(run)).rejects.toThrow(/adb not found/);
  });

  it('passes the phone state message through, so unauthorized is not called No phone found', async () => {
    const { run } = fakeRun([{ stdout: 'List of devices attached\nPHONE\tunauthorized\n' }]);
    await expect(androidPrepare(run)).rejects.toThrow(/not authorised/);
  });
});
