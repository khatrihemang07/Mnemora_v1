import type { Counts, PairingError, PairingView } from '../../lib/ipc/bindings';
import { nameList } from './pause';

/** A failure as the sheet words it: what happened, then what to do about it. */
export interface PairProblem {
  title: string;
  fix: string;
  /** The code is no longer valid: the person has to ask for a new one on the other device. */
  deadCode: boolean;
}

/** Shown in place of the code while the phone is on mobile data and "Allow over mobile data" is off. */
export const PAIRING_PAUSED = {
  title: 'Pairing is paused on mobile data.',
  fix: 'Connect to Wi-Fi, or turn on Allow over mobile data in Settings › Sync, then try again.',
};

const OTHER_DEVICE = 'the other device';

const triesLeft = (n: number) => `${n} ${n === 1 ? 'try' : 'tries'} left`;

/** Words for a pairing failure. `typing` is true on the device that types the code. */
export function problem(error: PairingError, typing: boolean): PairProblem {
  switch (error.kind) {
    case 'wrong_code':
      return {
        title: 'That code didn’t match.',
        fix:
          error.tries_left > 0
            ? `${triesLeft(error.tries_left)}. Check the code on ${error.name}.`
            : `It no longer works. Press New code on ${error.name}, then type the new one.`,
        deadCode: error.tries_left === 0,
      };
    case 'code_expired':
      return typing
        ? {
            title: 'That code has expired.',
            fix: `Press New code on ${error.name ?? OTHER_DEVICE} and type the new one.`,
            deadCode: true,
          }
        : { title: 'This code expired.', fix: 'Press New code to get another.', deadCode: true };
    case 'tries_used':
      return {
        title: 'That code was typed wrong 3 times.',
        fix: 'It no longer works. Press New code to get another.',
        deadCode: true,
      };
    case 'declined':
      return {
        title: 'Pairing was declined.',
        fix: `Deny was pressed on ${error.name || OTHER_DEVICE}. To try again, show a new code there.`,
        deadCode: true,
      };
    case 'cancelled':
      return {
        title: `Pairing was cancelled on ${error.name ?? OTHER_DEVICE}.`,
        fix: 'Start again when you’re ready.',
        deadCode: true,
      };
    case 'different_environment':
      return {
        title: 'One is Mnemora, the other is Mnemora Sandbox.',
        fix: 'They never pair with each other. Open the same one on both devices and try again.',
        deadCode: false,
      };
    case 'version_mismatch': {
      const who = error.older === 'me' ? 'this device' : error.name;
      // The Modules that are older, as the Paused banner names them.
      const modules = error.older === 'me' ? error.older_here : error.older_there;
      return {
        title:
          error.older === 'both'
            ? 'Update Mnemora on both devices first.'
            : modules.length > 0
              ? `Update Mnemora on ${who} first (it has an older ${nameList(modules)}).`
              : `Update Mnemora on ${who} first.`,
        fix:
          error.older === 'me'
            ? `${error.name} has newer data. Update this device, then pair again.`
            : error.older === 'them'
              ? `Its data is older than this device’s. Update it, then pair again.`
              : error.older_here.length > 0 && error.older_there.length > 0
                ? `${error.name} has an older ${nameList(error.older_there)} and this device an older ${nameList(error.older_here)}. Update both, then pair again.`
                : 'Each has something the other lacks. Update both, then pair again.',
        deadCode: false,
      };
    }
    case 'different_groups':
      return {
        title: 'Both devices already sync with other devices.',
        fix: 'On one of them, open Settings › Sync › Leave and forget all devices, then pair again.',
        deadCode: false,
      };
    case 'nothing_nearby':
      return {
        title: 'No device waiting nearby.',
        fix: 'Make sure both are on the same Wi-Fi, or pair by address.',
        deadCode: false,
      };
    case 'connection_lost':
      return {
        title: 'Lost the connection.',
        fix: 'Check that both devices are on the same network, then try again.',
        deadCode: false,
      };
    case 'address_not_found':
      return {
        title: 'That name wasn’t found.',
        fix: 'Check the spelling, and that Tailscale is on for both devices.',
        deadCode: false,
      };
    case 'address_unreachable':
      return {
        title: 'Nothing answered at that address.',
        fix: 'Check the address, that Mnemora is open on the other device with a code showing, and that both devices are on Tailscale.',
        deadCode: false,
      };
    case 'backup_failed':
      return {
        title: error.name
          ? `${error.name} couldn’t back up its data first.`
          : 'Couldn’t back up this device first.',
        fix: `Nothing was merged. Free some space${error.name ? ' there' : ''} and try again.`,
        deadCode: false,
      };
    case 'busy':
      return {
        title: 'That device is pairing with another one.',
        fix: 'Wait a moment and try again.',
        deadCode: false,
      };
    case 'not_ready':
      return {
        title: 'Sync is still starting.',
        fix: 'Try again in a moment.',
        deadCode: false,
      };
  }
}

/** One cell of the merge preview: a number and what it counts. */
export interface Stat {
  n: number;
  label: string;
}

/** The six numbers of the merge preview, the other device's first (prototype E7). */
export function previewStats(there: Counts, here: Counts): Stat[] {
  const row = (c: Counts, where: string): Stat[] => [
    { n: c.entries, label: `${c.entries === 1 ? 'Entry' : 'Entries'} ${where}` },
    { n: c.tasks, label: `${c.tasks === 1 ? 'Task' : 'Tasks'} ${where}` },
    { n: c.lists, label: `${c.lists === 1 ? 'List' : 'Lists'} ${where}` },
  ];
  return [...row(there, 'there'), ...row(here, 'here')];
}

/** `1:54` from milliseconds, rounding up so the ring and the text hit zero together. */
export function clock(ms: number): string {
  const total = Math.max(0, Math.ceil(ms / 1000));
  return `${Math.floor(total / 60)}:${String(total % 60).padStart(2, '0')}`;
}

/** The share of the window still left, 0 to 1. */
export function fractionLeft(leftMs: number, lifeMs: number): number {
  return lifeMs <= 0 ? 0 : Math.min(1, Math.max(0, leftMs / lifeMs));
}

/** How long is left of a countdown that was `left_ms` when it arrived `elapsedMs` ago. */
export function remaining(view: PairingView, elapsedMs: number): number {
  return view.state === 'showing' || view.state === 'asking'
    ? Math.max(0, view.left_ms - elapsedMs)
    : 0;
}

/** Whether the view is one where the sheet has pairing in progress (leaving needs a cancel). */
export function isBusy(view: PairingView): boolean {
  return view.state !== 'idle' && view.state !== 'paired' && view.state !== 'failed';
}
