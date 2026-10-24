// Maps one keypress to an action. Raw mode delivers Ctrl-C as a character.

import { TARGET_LIST } from './targets.mjs';

export const ACTION = Object.freeze({
  WEB: 'web',
  LAUNCH: 'launch',
  RELAUNCH: 'relaunch',
  STOP: 'stop',
  HELP: 'help',
  QUIT: 'quit',
});

const CORE_KEYS = Object.freeze({
  w: ACTION.WEB,
  r: ACTION.RELAUNCH,
  x: ACTION.STOP,
  '?': ACTION.HELP,
  '\u0003': ACTION.QUIT,
});

/** `{ action }`, or `{ action: LAUNCH, target }` for a target key, or null for any other key. */
export function keyToAction(key) {
  if (Object.hasOwn(CORE_KEYS, key)) {
    return { action: CORE_KEYS[key] };
  }
  const target = TARGET_LIST.find((candidate) => candidate.key === key);
  return target ? { action: ACTION.LAUNCH, target: target.id } : null;
}
