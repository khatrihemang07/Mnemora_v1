// Command-line flags for scripts/dev.mjs. Target flags come from the registry.

import { TARGET_LIST } from './targets.mjs';

const SANDBOX_FLAG = '--sandbox';
const WEB_FLAG = '--web';

export function usage() {
  const flags = TARGET_LIST.map((target) => `[--${target.id}]`).join(' ');
  return `Usage: node scripts/dev.mjs [--web] ${flags} [--sandbox]`;
}

/**
 * Reads the flags. An unknown flag is an error, never silently ignored. `launch` lists
 * the native targets to start once Vite is up, in the order given, without duplicates.
 */
export function parseArgs(argv) {
  const options = { sandbox: false, web: false, launch: [] };
  for (const arg of argv) {
    if (arg === SANDBOX_FLAG) {
      options.sandbox = true;
      continue;
    }
    if (arg === WEB_FLAG) {
      options.web = true;
      continue;
    }
    const target = TARGET_LIST.find((candidate) => arg === `--${candidate.id}`);
    if (!target) {
      throw new Error(`Unknown option: ${arg}\n${usage()}`);
    }
    if (!options.launch.includes(target.id)) {
      options.launch.push(target.id);
    }
  }
  return options;
}
