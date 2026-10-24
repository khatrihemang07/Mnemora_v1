import { describe, expect, it } from 'vitest';
import { DISABLE_BEFORE_DEV } from './commands.mjs';
import { ENVIRONMENT, environmentName, TARGET_LIST, TARGETS } from './targets.mjs';

describe('target registry', () => {
  it('is frozen, so a caller cannot change a target at runtime', () => {
    expect(Object.isFrozen(TARGETS)).toBe(true);
    expect(Object.isFrozen(TARGETS.mac)).toBe(true);
    expect(Object.isFrozen(ENVIRONMENT)).toBe(true);
  });

  it('gives every target an id, label, key and build-end marker', () => {
    for (const target of TARGET_LIST) {
      expect(target.id).toEqual(expect.any(String));
      expect(target.label).toEqual(expect.any(String));
      expect(target.key).toHaveLength(1);
      expect(target.endsBuild).toBeInstanceOf(RegExp);
      expect(TARGETS[target.id]).toBe(target);
    }
  });

  it('keys the target id to its own flag name', () => {
    expect(TARGET_LIST.map((target) => target.id)).toEqual(['mac', 'android']);
  });

  it('marks the Mac build end at cargo Finished or Running, and Android at Installing APK', () => {
    expect(TARGETS.mac.endsBuild.test('    Finished `dev` profile [unoptimized] in 3s')).toBe(true);
    expect(TARGETS.mac.endsBuild.test('     Running `target/debug/mnemora-app`')).toBe(true);
    expect(TARGETS.mac.endsBuild.test('   Compiling mnemora-core v0.1.0')).toBe(false);
    expect(TARGETS.android.endsBuild.test("Installing APK 'app-arm64-debug.apk'")).toBe(true);
    expect(TARGETS.android.endsBuild.test('BUILD SUCCESSFUL in 41s')).toBe(false);
  });
});

describe('command for each target and Environment', () => {
  const disable = ['--config', DISABLE_BEFORE_DEV];

  it('Mac in Production runs plain tauri dev with beforeDevCommand disabled', () => {
    const cmd = TARGETS.mac.command(TARGETS.mac.usesSandbox(false));
    expect(cmd.command).toBe('cargo');
    expect(cmd.args).toEqual(['tauri', 'dev', ...disable]);
    expect(cmd.env).toEqual({ CARGO_BUILD_JOBS: '2' });
  });

  it('Mac in Sandbox passes the sandbox config and feature', () => {
    const cmd = TARGETS.mac.command(TARGETS.mac.usesSandbox(true));
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

  it('Android is always Sandbox, whatever the Mac Environment is', () => {
    for (const sandbox of [false, true]) {
      const android = TARGETS.android;
      expect(android.usesSandbox(sandbox)).toBe(true);
      const cmd = android.command(android.usesSandbox(sandbox));
      expect(cmd.command).toBe('cargo');
      expect(cmd.args).toEqual(['tauri', 'android', 'dev', '--features', 'sandbox', ...disable]);
      expect(cmd.env).toEqual({ CARGO_BUILD_JOBS: '2', MNEMORA_ANDROID_SANDBOX: '1' });
    }
  });

  it('every target and Environment gets two cargo jobs, never more', () => {
    for (const target of TARGET_LIST) {
      for (const sandbox of [false, true]) {
        const cmd = target.command(target.usesSandbox(sandbox));
        expect(cmd.env.CARGO_BUILD_JOBS).toBe('2');
      }
    }
  });
});

describe('environmentName', () => {
  it('names Sandbox or Production', () => {
    expect(environmentName(true)).toBe('Sandbox');
    expect(environmentName(false)).toBe('Production');
  });
});
