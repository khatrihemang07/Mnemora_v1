import type { PeerRow } from '../../lib/ipc/bindings';

const MIN = 60_000;
const HOUR = 60 * MIN;

/**
 * How far a clock is out, in the words of "Pixel 8's clock is 3 h ahead": minutes under an hour,
 * hours (with the minutes while under ten hours), then whole hours up to two days, then days.
 */
export function durationWords(ms: number): string {
  const minutes = Math.max(1, Math.round(ms / MIN));
  if (minutes < 60) return `${minutes} min`;
  if (minutes < 600) {
    const rest = minutes % 60;
    return rest === 0 ? `${minutes / 60} h` : `${Math.floor(minutes / 60)} h ${rest} min`;
  }
  const hours = Math.round(ms / HOUR);
  return hours < 48 ? `${hours} h` : `${Math.round(hours / 24)} d`;
}

const MODULE_NAMES: Record<string, string> = {
  todo: 'Todo',
  journal: 'Journal',
  core: 'Settings',
  sync: 'Sync',
};

/** A Module as the person knows it. */
export function moduleName(id: string): string {
  return MODULE_NAMES[id] ?? id.charAt(0).toUpperCase() + id.slice(1);
}

/** "Todo", "Todo and Journal", "Todo, Journal and Settings". */
export function nameList(modules: string[]): string {
  const names = modules.map(moduleName);
  if (names.length <= 1) return names.join('');
  return `${names.slice(0, -1).join(', ')} and ${names[names.length - 1]}`;
}

/** What to tell the person about a device syncing is paused with, or null when it is not. */
export interface PauseBanner {
  title: string;
  text: string;
}

/** The banner on Devices for a paused device (cards D8–D10, "Devices (paused)"). */
export function pauseBanner(peer: Pick<PeerRow, 'name' | 'pause'>): PauseBanner | null {
  const pause = peer.pause;
  if (pause === null) return null;
  const title = `Paused with ${peer.name}`;
  if (pause.kind === 'stamped') {
    return {
      title,
      text: `${peer.name} has changes made while a clock was set ahead. Syncing resumes by itself in ${durationWords(pause.resumes_in_ms)}.`,
    };
  }
  if (pause.kind === 'clock') {
    const owner = pause.here ? 'This device' : peer.name;
    return {
      title,
      text: `${owner}'s clock is ${durationWords(pause.ahead_ms)} ahead. Fix the time to keep syncing.`,
    };
  }
  const { older_here: here, older_there: there } = pause;
  if (here.length > 0 && there.length > 0) {
    return {
      title,
      text: `Update Mnemora on both devices to keep syncing: ${peer.name} has an older ${nameList(there)} and this device an older ${nameList(here)}.`,
    };
  }
  if (here.length > 0) {
    return {
      title,
      text: `Update Mnemora on this device to keep syncing: this device has an older ${nameList(here)}.`,
    };
  }
  return {
    title,
    text: `Update Mnemora on ${peer.name} to keep syncing: it has an older ${nameList(there)}.`,
  };
}
