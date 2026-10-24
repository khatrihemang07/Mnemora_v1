// The command line for each native build. Pure: nothing is started here.

/**
 * Tauri's beforeDevCommand is `pnpm dev`. Vite already runs under the menu, and a second
 * one would fail on the strict port. `--no-dev-server` only disables Tauri's static file
 * server, so it is not the fix.
 */
export const DISABLE_BEFORE_DEV = JSON.stringify({ build: { beforeDevCommand: '' } });

/**
 * Tauri merges build.features from the sandbox config too. --features is passed as well,
 * so a CLI that ignored the config still cannot start Sandbox on Production data.
 */
const SANDBOX_MAC_ARGS = ['--config', 'src-tauri/tauri.sandbox.conf.json', '--features', 'sandbox'];

const CARGO_ENV = Object.freeze({ CARGO_BUILD_JOBS: '2' });

/** The Mac app: `cargo tauri dev`, Sandbox config and feature only when `sandbox` is set. */
export function macCommand(sandbox) {
  return {
    command: 'cargo',
    args: ['tauri', 'dev', ...(sandbox ? SANDBOX_MAC_ARGS : []), '--config', DISABLE_BEFORE_DEV],
    env: { ...CARGO_ENV },
  };
}

/** The Android app. Always Sandbox data, so `sandbox` is ignored. */
export function androidCommand() {
  return {
    command: 'cargo',
    args: ['tauri', 'android', 'dev', '--features', 'sandbox', '--config', DISABLE_BEFORE_DEV],
    env: { ...CARGO_ENV, MNEMORA_ANDROID_SANDBOX: '1' },
  };
}
