// The rules for the dev port: refuse when it is taken, and wait until Vite answers on it.
// The message is pure; the two probes are the I/O shell around it.

import net from 'node:net';
import { PORT_RETRY_MS } from './config.mjs';
import { runCommand } from './proc.mjs';

export function portInUseMessage(port) {
  return `Port ${port} is already in use: a dev server is probably running (see 'pgrep -fl vite'). Stop it first.`;
}

/** True when something listens on the port. Without lsof the answer is no, as the old Makefile check was. */
export async function isPortListening(port) {
  const result = await runCommand('lsof', [`-iTCP:${port}`, '-sTCP:LISTEN']).done.catch(() => ({
    code: 1,
  }));
  return result.code === 0;
}

/** Resolves true once localhost:port accepts a connection; false on timeout or when `isCancelled()`. */
export function waitForPort(port, { timeoutMs, isCancelled }) {
  const deadline = Date.now() + timeoutMs;
  return new Promise((resolve) => {
    const attempt = () => {
      if (isCancelled()) return resolve(false);
      const socket = net.connect({ host: 'localhost', port });
      socket.once('connect', () => {
        socket.destroy();
        resolve(true);
      });
      socket.once('error', () => {
        socket.destroy();
        if (Date.now() > deadline) return resolve(false);
        setTimeout(attempt, PORT_RETRY_MS);
      });
    };
    attempt();
  });
}
