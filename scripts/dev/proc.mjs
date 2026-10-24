// Child processes: short commands, the native build (its output read line by line), the
// process-tree kill and the Vite dev server. Decides nothing; the controller does that.

import { spawn } from 'node:child_process';
import { KILL_GRACE_MS } from './config.mjs';
import { descendantPids } from './process-tree.mjs';

/**
 * Runs a short command. `done` resolves { code, stdout, stderr } (code is null when
 * the command was killed) and rejects only when the command cannot be started.
 */
export function runCommand(command, args) {
  const child = spawn(command, args, { stdio: ['ignore', 'pipe', 'pipe'] });
  let stdout = '';
  let stderr = '';
  child.stdout.setEncoding('utf8');
  child.stdout.on('data', (chunk) => {
    stdout += chunk;
  });
  child.stderr.setEncoding('utf8');
  child.stderr.on('data', (chunk) => {
    stderr += chunk;
  });
  const done = new Promise((resolve, reject) => {
    child.on('error', reject);
    child.on('close', (code) => resolve({ code, stdout, stderr }));
  });
  return { done, cancel: () => child.kill('SIGTERM') };
}

function splitLines(stream, onLine) {
  stream.setEncoding('utf8');
  let buffer = '';
  stream.on('data', (chunk) => {
    const lines = (buffer + chunk).split(/\r\n|\n|\r/);
    buffer = lines.pop();
    for (const line of lines) onLine(line);
  });
  stream.on('end', () => {
    if (buffer) onLine(buffer);
    buffer = '';
  });
}

/**
 * Starts a native build with the given command. Output goes to `onLine` line by line;
 * `onExit(code, signal, error)` runs once the process ends, or with `error` if it never started.
 */
export function spawnNative({ command, args, env }, { onLine, onExit }) {
  const child = spawn(command, args, {
    env: { ...process.env, ...env },
    stdio: ['ignore', 'pipe', 'pipe'],
  });
  splitLines(child.stdout, onLine);
  splitLines(child.stderr, onLine);
  child.on('error', (error) => onExit(null, null, error.message));
  child.on('exit', (code, signal) => onExit(code, signal));
  return { pid: child.pid };
}

/** Kills a process and everything under it: children first, then KILL any survivor. */
export async function killTree(pid) {
  const ps = await runCommand('ps', ['-axo', 'pid=,ppid=']).done.catch(() => ({ stdout: '' }));
  const pids = [...descendantPids(ps.stdout, pid), pid];
  for (const target of pids) {
    try {
      process.kill(target, 'SIGTERM');
    } catch {
      // Already gone.
    }
  }
  await new Promise((resolve) => setTimeout(resolve, KILL_GRACE_MS));
  for (const target of pids) {
    try {
      process.kill(target, 0);
      process.kill(target, 'SIGKILL');
    } catch {
      // Already gone.
    }
  }
}

/** Starts Vite with the same command Tauri's beforeDevCommand would run. */
export function startVite(onExit) {
  const child = spawn('pnpm', ['dev'], { stdio: ['ignore', 'inherit', 'inherit'] });
  child.on('exit', (code, signal) => onExit(code, signal, null));
  child.on('error', (error) => onExit(null, null, error.message));
  return child;
}

/** Opens the URL in the default browser. */
export function openUrl(url, log) {
  const opener = spawn('open', [url], { stdio: 'ignore' });
  opener.on('error', () => log(`Could not open a browser. Open ${url} yourself.`));
}
