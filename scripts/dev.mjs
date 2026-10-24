#!/usr/bin/env node
// Entry for `make dev`, `make sandbox`, `make android-dev` and `pnpm dev:menu`. Starts Vite,
// then sends flags and keys to the build controller. The rules and I/O live in scripts/dev/.

import { parseArgs } from './dev/args.mjs';
import { DEV_PORT, DEV_URL, VITE_READY_TIMEOUT_MS } from './dev/config.mjs';
import { createController } from './dev/controller.mjs';
import { ACTION, keyToAction } from './dev/keys.mjs';
import { isPortListening, portInUseMessage, waitForPort } from './dev/port.mjs';
import { killTree, openUrl, runCommand, spawnNative, startVite } from './dev/proc.mjs';
import { attachKeys, log, restoreTerminal } from './dev/terminal.mjs';

let options;
try {
  options = parseArgs(process.argv.slice(2));
} catch (error) {
  console.error(error.message);
  process.exit(2);
}

const keysAvailable = Boolean(process.stdin.isTTY);
let controller = null;
let vite = null;
let quitting = false;

/** Stops every native app, then Vite, then exits. Safe to call more than once. */
async function quit(code) {
  if (quitting) return;
  quitting = true;
  restoreTerminal();
  if (controller) await controller.shutdown();
  if (vite) await killTree(vite.pid);
  log('Stopped.');
  process.exit(code);
}

function onKey(key) {
  const action = keyToAction(key);
  if (!action) return;
  if (action.action === ACTION.QUIT) {
    quit(0);
    return;
  }
  controller.dispatch(action);
}

async function main() {
  if (await isPortListening(DEV_PORT)) {
    log(portInUseMessage(DEV_PORT));
    process.exit(1);
  }

  controller = createController({
    sandbox: options.sandbox,
    keysAvailable,
    deps: {
      spawnNative,
      run: runCommand,
      killTree,
      openUrl: (url) => openUrl(url, log),
    },
    log,
  });
  if (!keysAvailable) log(controller.menu());

  vite = startVite((code, signal, error) => {
    if (error) log(`Could not start Vite: ${error}`);
    if (!quitting) {
      log(`Vite stopped (exit ${code ?? signal ?? 'unknown'}).`);
      quit(code ?? 1);
    }
  });

  const ready = await waitForPort(DEV_PORT, {
    timeoutMs: VITE_READY_TIMEOUT_MS,
    isCancelled: () => quitting,
  });
  if (quitting) return;
  if (!ready) {
    log(`Vite did not start on port ${DEV_PORT} within ${VITE_READY_TIMEOUT_MS / 1000} s.`);
    await quit(1);
    return;
  }
  log(`Vite is up at ${DEV_URL}`);

  if (keysAvailable) attachKeys(onKey);
  if (options.web) controller.dispatch({ action: ACTION.WEB });
  for (const target of options.launch) {
    controller.dispatch({ action: ACTION.LAUNCH, target });
  }
  if (keysAvailable) log(controller.menu());
}

process.on('SIGINT', () => quit(130));
process.on('SIGTERM', () => quit(143));
process.on('SIGHUP', () => quit(129));
process.on('exit', restoreTerminal);

main().catch((error) => {
  log(`dev: ${error.message}`);
  quit(1);
});
