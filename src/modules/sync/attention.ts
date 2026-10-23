import type { Attention } from '../../lib/ipc/bindings';
import type { ModuleBadge } from '../../lib/router';
import { relativeTime } from '../../lib/time/relative';

/**
 * Sync's badge in the desktop sidebar (B7): the count of conflicts that need you, nothing at zero,
 * and an amber "!" while sync is paused (a paused sync outranks the count: nothing will arrive
 * until it is dealt with). The phone tab bar never shows it (B8).
 */
export function badgeOf(a: Attention | undefined): ModuleBadge | null {
  if (!a) return null;
  if (a.paused.length > 0) {
    return { count: 0, alert: false, tone: 'warn', text: '!', label: 'Sync is paused' };
  }
  if (a.conflicts === 0) return null;
  return {
    count: a.conflicts,
    alert: false,
    tone: 'sync',
    label: `${a.conflicts} sync ${a.conflicts === 1 ? 'conflict needs' : 'conflicts need'} you`,
  };
}

const plural = (n: number, one: string, many: string) => `${n} ${n === 1 ? one : many}`;

/** "A and B", "A, B and C". */
function names(list: string[]): string {
  if (list.length <= 1) return list[0] ?? '';
  return `${list.slice(0, -1).join(', ')} and ${list[list.length - 1]}`;
}

/**
 * The Home card's one line (B11): one clause per issue, then when the last sync was.
 * "2 conflicts to review · Pixel 8 synced 2 min ago".
 */
export function homeCardLine(a: Attention, now: Date = new Date()): string {
  const parts: string[] = [];
  if (a.conflicts > 0) parts.push(`${plural(a.conflicts, 'conflict', 'conflicts')} to review`);
  if (a.paused.length > 0) parts.push(`Paused with ${names(a.paused)}`);
  for (const u of a.unseen) parts.push(`${u.name} not seen for ${plural(u.days, 'day', 'days')}`);
  // Not for a device already named above: "Pixel 8 not seen for 16 days · Pixel 8 synced 3 min ago".
  const named = [...a.paused, ...a.unseen.map((u) => u.name)];
  if (a.last_synced && !named.includes(a.last_synced.device)) {
    parts.push(`${a.last_synced.device} synced ${relativeTime(new Date(a.last_synced.at), now)}`);
  }
  return parts.join(' · ');
}

/** Where Review goes: the conflicts if any wait, otherwise the Devices page. */
export function reviewPath(a: Attention): '/sync/conflicts' | '/sync' {
  return a.conflicts > 0 ? '/sync/conflicts' : '/sync';
}

/** The note beside ⌘K's "Open Sync conflicts". */
export function paletteHint(a: Attention | undefined): string | null {
  return a && a.conflicts > 0 ? `${a.conflicts} waiting` : null;
}

/** The toast for conflicts that just arrived (F15): "1 sync conflict from Pixel 8". */
export function newConflictsText(count: number, device: string | null): string {
  const what = `${count} sync ${count === 1 ? 'conflict' : 'conflicts'}`;
  return device ? `${what} from ${device}` : what;
}
