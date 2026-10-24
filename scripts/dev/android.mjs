// Android preparation: check the phone, then forward the dev server port to it. The forward is
// undone when the build ends (see the undo command the controller runs).

import { parseAdbDevices, pickPhone } from './adb.mjs';
import { DEV_PORT } from './config.mjs';

const PORT_SPEC = `tcp:${DEV_PORT}`;

/** Message for a failed `adb reverse`. The first stderr line is usually the reason. */
export function reverseFailure(code, stderr) {
  const detail = stderr.trim().split('\n')[0];
  const reason = detail ? `: ${detail}` : '';
  return `adb reverse failed (exit ${code})${reason}. The phone cannot reach Vite on port ${DEV_PORT}; check the cable, then press a.`;
}

/**
 * `run(command, args)` resolves { code, stdout, stderr } and rejects only when the command
 * cannot be started. Returns the undo command on success; throws a message for the user otherwise.
 */
export async function androidPrepare(run) {
  const listing = await run('adb', ['devices']).catch(() => {
    throw new Error('adb not found. Install the Android platform tools (make setup).');
  });
  if (listing.code !== 0) {
    throw new Error(
      `adb devices failed (exit ${listing.code}). Check the Android platform tools (make setup).`,
    );
  }
  const serial = pickPhone(parseAdbDevices(listing.stdout));

  const forward = await run('adb', ['-s', serial, 'reverse', PORT_SPEC, PORT_SPEC]);
  if (forward.code !== 0) {
    throw new Error(reverseFailure(forward.code, forward.stderr));
  }
  return { command: 'adb', args: ['-s', serial, 'reverse', '--remove', PORT_SPEC] };
}
