/**
 * What the paired devices are doing, for the browser mock: the status words, "synced 2 min ago",
 * last session counts, the route, and a simulated sync with progress, so the Devices rows can be
 * exercised in the dev browser. Mirrors `enrich` in crates/sync/src/devices.rs (reachable devices
 * first, then the most recently seen) and the `sync_now` command in src-tauri/src/sync.rs.
 *
 * In the console: `__syncMock.setPeer('syncing' | 'up_to_date' | 'not_nearby' | 'paused', name?, daysAway?)`
 * puts "Pixel 8 (simulated)" (or the device called `name`) in a state; 'syncing' stays at
 * "120 of 340 changes" so the row can be looked at. "Sync now" runs a short simulated sync with every
 * device that is reachable.
 */
import type { PeerRow, PeerState, Progress, Route } from './bindings';

/** A paired device without its live state. */
export type PairedBase = Pick<PeerRow, 'id' | 'name' | 'platform' | 'paired_at'>;

interface Sim {
  state: PeerState;
  seenAt: number | null;
  syncedAt: number | null;
  lastIn: number | null;
  lastOut: number | null;
  route: Route | null;
  progress: Progress | null;
}

const MIN = 60_000;
const DAY = 24 * 60 * MIN;
/** The size of a simulated sync, and how fast it runs. */
const TOTAL = 340;
const STEP = 60;
const STEP_MS = 350;

function initial(peer: PairedBase, now: number): Sim {
  // The two devices of `?paired=2` look like the prototype's: one around, one away on Tailscale.
  if (peer.id === 'mock-air') {
    return {
      state: 'not_nearby',
      seenAt: now - 3 * DAY,
      syncedAt: now - 3 * DAY,
      lastIn: 40,
      lastOut: 2,
      route: 'tailscale',
      progress: null,
    };
  }
  const fresh = !peer.id.startsWith('mock-pixel');
  return {
    state: 'up_to_date',
    seenAt: now - (fresh ? 0 : 2 * MIN),
    syncedAt: now - (fresh ? 0 : 2 * MIN),
    lastIn: fresh ? null : 12,
    lastOut: fresh ? null : 3,
    route: 'wifi',
    progress: null,
  };
}

export function createLiveMock(peers: PairedBase[], emit: () => void, enabled: () => boolean) {
  const sims = new Map<string, Sim>();
  const timers = new Map<string, ReturnType<typeof setTimeout>>();
  const sim = (peer: PairedBase): Sim => {
    let s = sims.get(peer.id);
    if (!s) {
      s = initial(peer, Date.now());
      sims.set(peer.id, s);
    }
    return s;
  };

  function finish(peer: PairedBase) {
    const s = sim(peer);
    const now = Date.now();
    Object.assign(s, {
      state: 'up_to_date',
      progress: null,
      seenAt: now,
      syncedAt: now,
      lastIn: TOTAL - 40,
      lastOut: 40,
    } satisfies Partial<Sim>);
    timers.delete(peer.id);
    emit();
  }

  function run(peer: PairedBase, done = 0) {
    const s = sim(peer);
    s.state = 'syncing';
    s.progress = { done, total: TOTAL };
    emit();
    const next = done + STEP;
    timers.set(
      peer.id,
      setTimeout(() => (next >= TOTAL ? finish(peer) : run(peer, next)), STEP_MS),
    );
  }

  function cancel(id: string) {
    clearTimeout(timers.get(id));
    timers.delete(id);
  }

  return {
    /** The rows `sync_devices` lists. */
    rows(): PeerRow[] {
      const reachable = (r: PeerRow) => r.state !== 'not_nearby';
      return peers
        .map((p): PeerRow => {
          const s = sim(p);
          return {
            ...p,
            state: s.state,
            last_seen_at: s.seenAt,
            last_synced_at: s.syncedAt,
            last_in: s.lastIn,
            last_out: s.lastOut,
            route: s.route,
            progress: s.state === 'syncing' ? s.progress : null,
          };
        })
        .sort(
          (a, b) =>
            Number(reachable(b)) - Number(reachable(a)) ||
            (b.last_seen_at ?? 0) - (a.last_seen_at ?? 0) ||
            b.paired_at - a.paired_at ||
            a.name.localeCompare(b.name),
        );
    },

    /** "Sync now": every reachable device runs a simulated sync. */
    syncNow() {
      if (!enabled()) return;
      for (const p of peers) {
        const s = sim(p);
        if ((s.state === 'up_to_date' || s.state === 'syncing') && !timers.has(p.id)) run(p);
      }
    },

    /** Console helper: put a device in a state and leave it there. */
    setPeer(state: PeerState, name = 'Pixel 8 (simulated)', daysAway = 0) {
      const peer = peers.find((p) => p.name === name) ?? peers[0];
      if (!peer) return;
      cancel(peer.id);
      const s = sim(peer);
      s.state = state;
      s.progress = state === 'syncing' ? { done: 120, total: TOTAL } : null;
      // A device that is away was last seen `daysAway` days ago (16 shows the Home card).
      if (state !== 'not_nearby') s.seenAt = Date.now();
      else if (daysAway > 0) {
        s.seenAt = Date.now() - daysAway * DAY;
        s.syncedAt = s.seenAt;
      }
      emit();
    },

    /** Forgets everything (the group was left) and stops any simulated sync. */
    reset() {
      for (const id of [...timers.keys()]) cancel(id);
      sims.clear();
    },
  };
}
