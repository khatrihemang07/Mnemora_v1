import type { PeerRow, PeerState, Progress, Route } from '../../lib/ipc/bindings';
import { clockParts, type TimeFormat } from '../../lib/time/format';
import { absoluteDay, relativeTime } from '../../lib/time/relative';

/** The words a paired device's state is shown with (D3: plain). */
export const STATUS_WORDS: Record<PeerState, string> = {
  up_to_date: 'Up to date',
  syncing: 'Syncing…',
  not_nearby: 'Not nearby',
  paused_on_mobile_data: 'Paused on mobile data',
  paused: 'Paused',
  removed: 'Removed',
};

export function statusWord(state: PeerState): string {
  return STATUS_WORDS[state];
}

// U+2011 is a hyphen that never breaks the line ("Wi-" / "Fi" on a phone).
const ROUTE_WORDS: Record<Route, string> = { wifi: 'Wi‑Fi', tailscale: 'Tailscale' };

export function routeWord(route: Route): string {
  return ROUTE_WORDS[route];
}

/** "120 of 340 changes", shown after "Syncing…". Empty until the size of the sync is known. */
export function progressText(progress: Progress | null): string {
  if (progress === null || progress.total === 0) return '';
  const done = Math.min(progress.done, progress.total);
  return `${done} of ${progress.total} ${progress.total === 1 ? 'change' : 'changes'}`;
}

/** "12 in · 3 out": what the last session that moved anything moved. */
export function countsText(peer: Pick<PeerRow, 'last_in' | 'last_out'>): string | null {
  if (peer.last_in === null && peer.last_out === null) return null;
  return `${peer.last_in ?? 0} in · ${peer.last_out ?? 0} out`;
}

/** "synced 2 min ago" (relative under a week, then the date); a paused device reads "last synced …". */
export function syncedText(
  peer: Pick<PeerRow, 'last_synced_at' | 'state'>,
  now: Date = new Date(),
): string {
  if (peer.last_synced_at === null) return 'not synced yet';
  const when = relativeTime(new Date(peer.last_synced_at), now);
  return `${peer.state === 'paused' || peer.state === 'paused_on_mobile_data' ? 'last synced' : 'synced'} ${when}`;
}

/** The full date and time, for the hover on a relative time: "Wed 14 Oct, 15:05". */
export function absoluteTime(ms: number, format: TimeFormat, now: Date = new Date()): string {
  const at = new Date(ms);
  const { hm, period } = clockParts(at, format);
  return `${absoluteDay(at, now)}, ${hm}${period ? ` ${period}` : ''}`;
}

/**
 * The line under a device's name, as parts joined by "·" (D2): the status word, how long ago it
 * synced, the last session's counts (only while up to date) and how it was reached. A syncing
 * device reads "Syncing… 120 of 340 changes" instead.
 */
export function rowParts(peer: PeerRow, now: Date = new Date()): string[] {
  if (peer.state === 'syncing') {
    return [statusWord('syncing'), progressText(peer.progress)].filter(Boolean);
  }
  const parts = [statusWord(peer.state), syncedText(peer, now)];
  const counts = countsText(peer);
  if (peer.state === 'up_to_date' && counts !== null) parts.push(counts);
  if (peer.route !== null) parts.push(routeWord(peer.route));
  return parts;
}
