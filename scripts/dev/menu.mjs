// Text for the key menu and the one-line status. Pure: the controller passes the state in.

import { DEV_URL } from './config.mjs';
import { TARGET_LIST, TARGETS } from './targets.mjs';

const KEY_WIDTH = 9;

function line(key, text) {
  return `  ${key.padEnd(KEY_WIDTH)}${text}`;
}

/** One line, printed after every change: Environment, each target's state, the queue. */
export function formatStatus({ environment, states, queue }) {
  const targets = TARGET_LIST.map((target) => `${target.label}: ${states[target.id]}`);
  const waiting = queue.length > 0 ? queue.map((id) => TARGETS[id].label).join(', ') : 'none';
  return `Mnemora dev · ${environment} · ${targets.join(' · ')} · Queue: ${waiting}`;
}

/**
 * The full menu, printed on start and on `?`. `environments` maps each target to the
 * Environment it runs on, so every key line says what it builds.
 */
export function formatMenu({ environment, states, environments, queue, keysAvailable }) {
  if (!keysAvailable) {
    return `Keys unavailable (not a terminal): no key menu. Stop with Ctrl-C.\n${DEV_URL}`;
  }
  const launchLines = TARGET_LIST.map((target) =>
    line(target.key, `launch the ${target.label} app (${environments[target.id]})`),
  );
  return [
    `Mnemora dev · ${environment} · ${DEV_URL}`,
    line('w', 'open the web mock in your browser'),
    ...launchLines,
    line('r', 'relaunch the running or queued app(s)'),
    line('x', 'stop the native app(s), keep Vite'),
    line('?', 'show this menu'),
    line('Ctrl-C', 'quit everything'),
    formatStatus({ environment, states, queue }),
  ].join('\n');
}
