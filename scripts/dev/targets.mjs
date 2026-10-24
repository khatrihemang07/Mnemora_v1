// The registry of native targets. Adding one (iOS, a second desktop build) means one entry
// here plus its command in commands.mjs. Flags, keys, the menu, the queue and quit read
// this table, so nothing else needs a switch.

import { androidPrepare } from './android.mjs';
import { androidCommand, macCommand } from './commands.mjs';

export const ENVIRONMENT = Object.freeze({
  PRODUCTION: 'Production',
  SANDBOX: 'Sandbox',
});

export function environmentName(sandbox) {
  return sandbox ? ENVIRONMENT.SANDBOX : ENVIRONMENT.PRODUCTION;
}

/**
 * Fields:
 *  id          queue name and the flag suffix (--id)
 *  label       shown in messages and the menu
 *  key         the menu key that launches it
 *  usesSandbox(sandbox) which Environment this target runs on, given the Mac Environment
 *  command(sandbox)     the command line, as { command, args, env }
 *  endsBuild   a line that ends the build slot (the queue then starts the next build)
 *  prepare     optional async step before spawning; returns an undo command or null
 */
export const TARGETS = Object.freeze({
  mac: Object.freeze({
    id: 'mac',
    label: 'Mac',
    key: 'q',
    usesSandbox: (sandbox) => sandbox,
    command: macCommand,
    // cargo prints "Finished" when the build ends and "Running" when the app starts. Either
    // one frees the slot, so the running app does not hold it.
    endsBuild: /Finished `|Running `/,
    prepare: null,
  }),
  android: Object.freeze({
    id: 'android',
    label: 'Android',
    key: 'a',
    usesSandbox: () => true,
    command: androidCommand,
    // Gradle's "BUILD SUCCESSFUL" can appear more than once, so it is not used. AGP
    // prints "Installing APK" once, just before the install and launch.
    endsBuild: /Installing APK/,
    prepare: androidPrepare,
  }),
});

export const TARGET_LIST = Object.freeze(Object.values(TARGETS));
