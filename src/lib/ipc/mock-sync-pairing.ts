/**
 * Pairing for the browser mock: a simulated second device, "Pixel 8 (simulated)", so the whole
 * pairing flow and every failure can be exercised in the dev browser. Mirrors crates/sync/src/pair
 * (the code rules, the name rule, the phases) and the commands in src-tauri/src/sync.rs.
 *
 * Showing a code ("Pair a device"): after `delayMs` the simulated phone types the right code and asks
 * Allow or Deny. Change that from the console: `__syncMock.pixel = 'wrong' | 'never' | 'joins'`
 * (`wrong`: it mistypes 3 times, which kills the code; `never`: nothing happens until the code
 * expires; `joins`: it also brings two devices of its group). `__syncMock.delayMs` and
 * `__syncMock.lifeMs` (or `?paircode=SECONDS` in the address for a short-lived code) change the timing.
 *
 * Typing a code ("Enter a code") uses magic codes:
 *   12345678  right: waits for Allow, then pairs            11111111 (or any other)  wrong: 2, 1, then 0 tries left
 *   00000000  nothing found nearby                           20000000  the code expired
 *   30000000  declined                                       40000000  the other device is Production
 *   50000000  its data is older (50000001: this device's, 50000002: both)
 *   60000000  connection lost        70000000  different groups      80000000  busy
 *   90000000  right, but the name clashes ("Pixel 8 (simulated) (2)")
 *   91000000  right, and two devices of its group join too           99999999  cancelled from the other device
 */
import type { AppError, PairingError, PairingView, PeerState } from './bindings';
import type { PairedBase } from './mock-sync-live';

export interface PairingFlags {
  pixel: 'right' | 'wrong' | 'never' | 'joins';
  /** Until the simulated phone types the code, or mistypes it again. */
  delayMs: number;
  /** How long a code lives. */
  lifeMs: number;
  /** One step of the simulated exchange. */
  stepMs: number;
  /** Console helper (set by the Sync mock): puts a paired device in a state. */
  setPeer?: (state: PeerState, name?: string, daysAway?: number) => void;
  /** Console: a new conflict arrived from the simulated device (toast, if this device wants one). */
  newConflict?: () => void;
}

export const defaultPairingFlags = (): PairingFlags => ({
  pixel: 'right',
  delayMs: 4000,
  lifeMs: 120_000,
  stepMs: 900,
});

const PIXEL = 'Pixel 8 (simulated)';
const GROUP = ['MacBook Air (simulated)', 'iPad (simulated)'];
const MAX_TRIES = 3;

const fold = (name: string) =>
  name.normalize('NFD').replace(/\p{M}/gu, '').toLowerCase().split(/\s+/).filter(Boolean).join(' ');

/** Mirrors `unique_name` in crates/sync/src/peers.rs. */
export function uniqueName(wanted: string, taken: string[]): string {
  const used = new Set(taken.map(fold));
  if (!used.has(fold(wanted))) return wanted;
  for (let n = 2; ; n++) {
    const suffix = ` (${n})`;
    const candidate = `${[...wanted]
      .slice(0, 40 - suffix.length)
      .join('')
      .trimEnd()}${suffix}`;
    if (!used.has(fold(candidate))) return candidate;
  }
}

/** Mirrors `normalise` in crates/sync/src/pair/code.rs. */
export function normaliseCode(input: string): string | null {
  const digits = input.replace(/[\s-]/g, '');
  return /^[0-9]{8}$/.test(digits) ? digits : null;
}

const display = (code: string) => `${code.slice(0, 4)} ${code.slice(4)}`;
const randomCode = () => String(Math.floor(Math.random() * 1e8)).padStart(8, '0');
const err = (message: string): AppError => ({ code: 'validation', message, details: null });

export const PAIRING_COMMANDS = [
  'sync_pair_state',
  'sync_pair_show',
  'sync_pair_new_code',
  'sync_pair_enter',
  'sync_pair_decide',
  'sync_pair_cancel',
];

interface Deps {
  peers: PairedBase[];
  emit: (view: PairingView) => void;
  myName: () => string;
  enabled: () => boolean;
  visible: () => boolean;
  flags: PairingFlags;
}

interface Shown {
  code: string;
  expires: number;
  tries: number;
  dead: PairingError | null;
}

export function createPairingMock(deps: Deps) {
  const { peers, flags } = deps;
  let view: PairingView = { state: 'idle' };
  let shown: Shown | null = null;
  let timers: ReturnType<typeof setTimeout>[] = [];
  let wrongStreak = 0;
  let joinGroup = false;

  const set = (next: PairingView) => {
    view = next;
    deps.emit(next);
  };
  const later = (ms: number, fn: () => void) => {
    timers.push(setTimeout(fn, ms));
  };
  const stop = () => {
    for (const t of timers) clearTimeout(t);
    timers = [];
    shown = null;
  };

  const showing = (): PairingView => {
    if (!shown) return view;
    if (shown.dead) return { state: 'failed', error: shown.dead };
    return {
      state: 'showing',
      code: display(shown.code),
      life_ms: flags.lifeMs,
      left_ms: Math.max(0, shown.expires - Date.now()),
      tries_left: shown.tries,
      discoverable: deps.visible(),
      notice: null,
    };
  };

  const current = (): PairingView =>
    shown && (view.state === 'showing' || view.state === 'failed') ? showing() : view;

  function pair(joined: string[], forceClash = false) {
    const taken = [deps.myName(), ...peers.map((p) => p.name)];
    if (forceClash) taken.push(PIXEL);
    const name = uniqueName(PIXEL, taken);
    const added = [name];
    peers.unshift({ id: `mock-${Date.now()}`, name, platform: 'phone', paired_at: Date.now() });
    const also: string[] = [];
    for (const extra of joined) {
      const stored = uniqueName(extra, [...taken, ...added]);
      added.push(stored);
      also.push(stored);
      peers.push({ id: `mock-${extra}`, name: stored, platform: 'desktop', paired_at: Date.now() });
    }
    stop();
    wrongStreak = 0;
    set({ state: 'paired', name, platform: 'phone', renamed: name !== PIXEL, also_joined: also });
  }

  function startCode() {
    stop();
    shown = {
      code: randomCode(),
      expires: Date.now() + flags.lifeMs,
      tries: MAX_TRIES,
      dead: null,
    };
    set(showing());
    later(flags.lifeMs, () => {
      if (!shown || shown.dead) return;
      shown.dead = { kind: 'code_expired', name: null };
      set(showing());
    });
    if (flags.pixel === 'wrong') mistype();
    else if (flags.pixel !== 'never') {
      joinGroup = flags.pixel === 'joins';
      later(flags.delayMs, ask);
    }
  }

  function mistype() {
    later(flags.delayMs, () => {
      if (!shown || shown.dead) return;
      shown.tries -= 1;
      if (shown.tries === 0) shown.dead = { kind: 'tries_used' };
      set(showing());
      if (shown.tries > 0) mistype();
    });
  }

  function ask() {
    if (!shown || shown.dead) return;
    set({
      state: 'asking',
      name: PIXEL,
      platform: 'phone',
      life_ms: flags.lifeMs,
      left_ms: Math.max(0, shown.expires - Date.now()),
    });
  }

  function typed(code: string) {
    const fail = (error: PairingError) => set({ state: 'failed', error });
    const after = (ms: number, fn: () => void) => later(ms, fn);
    switch (code) {
      case '12345678':
      case '90000000':
      case '91000000':
        set({ state: 'waiting', name: PIXEL });
        after(flags.stepMs * 1.5, () =>
          pair(code === '91000000' ? GROUP : [], code === '90000000'),
        );
        return;
      case '00000000':
        return fail({ kind: 'nothing_nearby' });
      case '20000000':
        return fail({ kind: 'code_expired', name: PIXEL });
      case '30000000':
        set({ state: 'waiting', name: PIXEL });
        after(flags.stepMs * 1.5, () => fail({ kind: 'declined', name: PIXEL }));
        return;
      case '40000000':
        return fail({ kind: 'different_environment', theirs: 'production' });
      case '50000000':
        return fail({ kind: 'version_mismatch', name: PIXEL, older: 'them' });
      case '50000001':
        return fail({ kind: 'version_mismatch', name: PIXEL, older: 'me' });
      case '50000002':
        return fail({ kind: 'version_mismatch', name: PIXEL, older: 'both' });
      case '60000000':
        return fail({ kind: 'connection_lost' });
      case '70000000':
        return fail({ kind: 'different_groups' });
      case '80000000':
        return fail({ kind: 'busy' });
      case '99999999':
        set({ state: 'waiting', name: PIXEL });
        after(flags.stepMs * 1.5, () => fail({ kind: 'cancelled', name: PIXEL }));
        return;
      default: {
        wrongStreak += 1;
        const tries_left = Math.max(0, MAX_TRIES - wrongStreak);
        if (tries_left === 0) wrongStreak = 0;
        return fail({ kind: 'wrong_code', tries_left, name: PIXEL });
      }
    }
  }

  const needSync = () => {
    if (!deps.enabled()) throw err('Sync is off on this device');
  };

  return {
    commands: PAIRING_COMMANDS,
    handle(cmd: string, a: Record<string, unknown>): unknown {
      switch (cmd) {
        case 'sync_pair_state':
          return JSON.parse(JSON.stringify(current()));
        case 'sync_pair_show':
          needSync();
          startCode();
          return null;
        case 'sync_pair_new_code':
          needSync();
          if (shown && view.state !== 'asking') startCode();
          return null;
        case 'sync_pair_enter': {
          needSync();
          const code = normaliseCode(String(a.code));
          if (!code) throw err('Type all 8 digits of the code');
          stop();
          set({ state: 'searching' });
          later(flags.stepMs, () => typed(code));
          return null;
        }
        case 'sync_pair_decide':
          needSync();
          if (view.state !== 'asking') return null;
          if (a.allow !== true) {
            stop();
            set({ state: 'idle' });
          } else {
            set({ state: 'finishing', name: PIXEL });
            later(flags.stepMs, () => pair(joinGroup ? GROUP : []));
          }
          return null;
        case 'sync_pair_cancel':
          stop();
          wrongStreak = 0;
          if (view.state !== 'idle') set({ state: 'idle' });
          return null;
        default:
          return undefined;
      }
    },
  };
}
