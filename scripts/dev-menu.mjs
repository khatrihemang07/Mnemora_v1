// Pure logic for scripts/dev.mjs: flags, keys, the native build queue and the
// cargo/adb command lines. No I/O here, so every rule is testable with Vitest.

export const DEV_PORT = 47821;

// Tauri's beforeDevCommand is `pnpm dev`. Our menu already runs Vite, so the native
// build must not start a second one (it would fail on the strict port). `--no-dev-server`
// is not the answer: it only disables Tauri's static file server.
export const DISABLE_BEFORE_DEV = JSON.stringify({ build: { beforeDevCommand: '' } });

// Tauri merges build.features from the sandbox config too; passing --features as well
// means a CLI that ignored the config could still never start Sandbox on Production data.
const SANDBOX_MAC_ARGS = ['--config', 'src-tauri/tauri.sandbox.conf.json', '--features', 'sandbox'];

const USAGE = 'Usage: scripts/dev.mjs [--web] [--mac] [--android] [--sandbox]';

const FLAGS = {
  '--sandbox': 'sandbox',
  '--web': 'web',
  '--mac': 'mac',
  '--android': 'android',
};

/** Reads the flags; a flag that is not known is an error, never silently ignored. */
export function parseArgs(argv) {
  const options = { sandbox: false, web: false, mac: false, android: false };
  for (const arg of argv) {
    const key = FLAGS[arg];
    if (!key) {
      throw new Error(`Unknown option: ${arg}\n${USAGE}`);
    }
    options[key] = true;
  }
  return options;
}

const KEY_ACTIONS = {
  w: 'web',
  q: 'mac',
  a: 'android',
  r: 'relaunch',
  x: 'stop',
  '?': 'help',
  '\u0003': 'quit',
};

/** Maps one keypress (raw mode delivers Ctrl-C as a character) to an action, or null. */
export function keyToAction(key) {
  return Object.hasOwn(KEY_ACTIONS, key) ? KEY_ACTIONS[key] : null;
}

/** Serials of phones that are online, from `adb devices` output. */
export function parseAdbDevices(text) {
  const serials = [];
  for (const line of text.split('\n')) {
    const [serial, state] = line.trim().split(/\s+/);
    if (serial && state === 'device' && !serial.startsWith('List')) {
      serials.push(serial);
    }
  }
  return serials;
}

/**
 * The line that ends a native build phase, so the next queued build may start.
 * Cargo prints "Finished" (or "Running" when the app starts). Gradle's end is
 * "BUILD SUCCESSFUL". If the marker never shows, the slot is freed when the child
 * exits, so the queue fails safe by staying serial.
 */
export function buildDoneMarker(target, line) {
  if (target === 'mac') {
    return /Finished `|Running `/.test(line);
  }
  return /BUILD SUCCESSFUL/.test(line);
}

/** The cargo or adb command for one native target, run with the given Environment. */
export function nativeCommand(target, { sandbox }) {
  const env = { CARGO_BUILD_JOBS: '2' };
  if (target === 'mac') {
    const args = [
      'tauri',
      'dev',
      ...(sandbox ? SANDBOX_MAC_ARGS : []),
      '--config',
      DISABLE_BEFORE_DEV,
    ];
    return { command: 'cargo', args, env };
  }
  // Android is always Sandbox data, as with `make android-dev`.
  return {
    command: 'cargo',
    args: ['tauri', 'android', 'dev', '--features', 'sandbox', '--config', DISABLE_BEFORE_DEV],
    env: { ...env, MNEMORA_ANDROID_SANDBOX: '1' },
  };
}

/**
 * Runs at most one native build at a time. `startJob(target, release)` must call
 * `release()` when its build phase is over (or its child has exited); the next
 * queued target then starts.
 */
export function createBuildQueue(startJob) {
  const pending = [];
  let slot = null;

  function pump() {
    if (slot || pending.length === 0) return;
    const current = { target: pending.shift(), released: false };
    slot = current;
    const release = () => {
      if (current.released) return;
      current.released = true;
      if (slot === current) slot = null;
      pump();
    };
    startJob(current.target, release);
  }

  return {
    /** Returns false when the target is already waiting. */
    enqueue(target) {
      if (pending.includes(target)) return false;
      pending.push(target);
      pump();
      return true;
    },
    /** Drops the builds that have not started; returns their targets. */
    cancelPending() {
      return pending.splice(0, pending.length);
    },
    buildingTarget() {
      return slot ? slot.target : null;
    },
    pendingTargets() {
      return [...pending];
    },
  };
}

/** Pids below `rootPid` in `ps -axo pid=,ppid=` output, children before parents. */
export function descendantPids(psText, rootPid) {
  const children = new Map();
  for (const line of psText.split('\n')) {
    const [pid, ppid] = line.trim().split(/\s+/).map(Number);
    if (!Number.isInteger(pid) || !Number.isInteger(ppid)) continue;
    if (!children.has(ppid)) children.set(ppid, []);
    children.get(ppid).push(pid);
  }
  const out = [];
  const visit = (pid) => {
    for (const child of children.get(pid) ?? []) {
      visit(child);
      out.push(child);
    }
  };
  visit(rootPid);
  return out;
}

const NATIVE_LABELS = { mac: 'Mac', android: 'Android' };

/** The key menu printed on start, on `?` and after each change. */
export function formatMenu({
  environment,
  port = DEV_PORT,
  natives,
  building,
  pending,
  keysAvailable,
}) {
  if (!keysAvailable) {
    return `Keys unavailable (not a terminal): Vite only. Stop with Ctrl-C.\nhttp://localhost:${port}`;
  }
  const status = Object.entries(NATIVE_LABELS).map(([target, label]) => {
    const state = building === target ? 'building' : (natives[target] ?? 'stopped');
    return `${label}: ${state}`;
  });
  const queue = pending.length > 0 ? pending.join(', ') : 'none';
  return [
    `Mnemora dev · ${environment} · http://localhost:${port}`,
    '  w        open the web mock in your browser',
    `  q        launch the Mac app (${environment})`,
    '  a        launch the Android app (Sandbox)',
    '  r        relaunch the running app(s)',
    '  x        stop the native app(s), keep Vite',
    '  ?        show this menu',
    '  Ctrl-C   quit everything',
    `${status.join(' · ')} · Queue: ${queue}`,
  ].join('\n');
}
