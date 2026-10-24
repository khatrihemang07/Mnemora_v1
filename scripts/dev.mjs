#!/usr/bin/env node
// Vite plus an Expo-style key menu. Starts only Vite; the Mac and Android apps are built
// on request (keys or flags), one native build at a time. Pure rules live in dev-menu.mjs.
// Run through `make dev` / `make sandbox` (port check, .env.local, tree kill) or `pnpm dev:menu`.

import { spawn } from 'node:child_process';
import net from 'node:net';
import {
  buildDoneMarker,
  createBuildQueue,
  DEV_PORT,
  descendantPids,
  formatMenu,
  keyToAction,
  nativeCommand,
  parseAdbDevices,
  parseArgs,
} from './dev-menu.mjs';

const LABELS = { mac: 'Mac', android: 'Android' };
const URL = `http://localhost:${DEV_PORT}`;
// Shorter than with-env.sh, which SIGKILLs this process one second after its own TERM.
const KILL_GRACE_MS = 500;
const PORT_WAIT_MS = 60_000;

let options;
try {
  options = parseArgs(process.argv.slice(2));
} catch (error) {
  console.error(error.message);
  process.exit(2);
}

const environment = options.sandbox ? 'Sandbox' : 'Production';
const keysAvailable = Boolean(process.stdin.isTTY);

/** target -> { child, release, restart } for each native process alive or starting. */
const natives = new Map();
let vite = null;
let shuttingDown = false;
let reverseSet = false;

const queue = createBuildQueue(startNative);

function log(message) {
  process.stdout.write(`${message}\n`);
}

function menuState() {
  return {
    environment,
    keysAvailable,
    natives: Object.fromEntries([...natives.keys()].map((target) => [target, 'running'])),
    building: queue.buildingTarget(),
    pending: queue.pendingTargets(),
  };
}

/** Runs a short command and collects its stdout; resolves with the exit code. */
function capture(command, args) {
  return new Promise((resolve, reject) => {
    const child = spawn(command, args, { stdio: ['ignore', 'pipe', 'ignore'] });
    let stdout = '';
    child.stdout.setEncoding('utf8');
    child.stdout.on('data', (chunk) => {
      stdout += chunk;
    });
    child.on('error', reject);
    child.on('close', (code) => resolve({ code, stdout }));
  });
}

async function prepareAndroid() {
  const adb = await capture('adb', ['devices']).catch(() => null);
  if (!adb) {
    throw new Error('adb not found. Install the Android platform tools (make setup).');
  }
  const devices = parseAdbDevices(adb.stdout);
  if (devices.length === 0) {
    throw new Error('No phone found. Connect it over USB with USB debugging on, then press a.');
  }
  if (devices.length > 1) {
    throw new Error('More than one phone is connected. Leave one connected, then press a.');
  }
  const port = `tcp:${DEV_PORT}`;
  await capture('adb', ['reverse', port, port]);
  reverseSet = true;
}

async function removeAndroidReverse() {
  if (!reverseSet) return;
  reverseSet = false;
  await capture('adb', ['reverse', '--remove', `tcp:${DEV_PORT}`]).catch(() => {});
}

/** Starts one native build; `release` frees the queue slot when its build phase is over. */
async function startNative(target, release) {
  const label = LABELS[target];
  try {
    if (target === 'android') {
      await prepareAndroid();
    }
  } catch (error) {
    log(`${label}: ${error.message}`);
    release();
    return;
  }

  const { command, args, env } = nativeCommand(target, { sandbox: options.sandbox });
  log(`${label}: building (${target === 'android' ? 'Sandbox' : environment})`);
  const child = spawn(command, args, {
    env: { ...process.env, ...env },
    stdio: ['ignore', 'pipe', 'pipe'],
  });
  const entry = { child, release, restart: false };
  natives.set(target, entry);

  let finished = false;
  const finish = (detail) => {
    if (finished) return;
    finished = true;
    if (natives.get(target) === entry) natives.delete(target);
    release();
    if (target === 'android') removeAndroidReverse();
    log(`${label}: stopped${detail ? ` (${detail})` : ''}`);
    if (entry.restart && !shuttingDown) {
      queue.enqueue(target);
    }
  };

  for (const stream of [child.stdout, child.stderr]) {
    stream.setEncoding('utf8');
    let buffer = '';
    stream.on('data', (chunk) => {
      const lines = (buffer + chunk).split(/\r\n|\n|\r/);
      buffer = lines.pop();
      for (const line of lines) {
        if (buildDoneMarker(target, line)) release();
        log(`[${target}] ${line}`);
      }
    });
  }
  child.on('error', (error) => finish(error.message));
  child.on('exit', (code, signal) => finish(signal ? `signal ${signal}` : `exit ${code}`));
}

function requestLaunch(target) {
  const label = LABELS[target];
  if (shuttingDown) return;
  if (natives.has(target) || queue.buildingTarget() === target) {
    log(`${label} is already running. r relaunches it.`);
    return;
  }
  if (queue.pendingTargets().includes(target)) {
    log(`${label} is already queued.`);
    return;
  }
  if (queue.buildingTarget() || queue.pendingTargets().length > 0) {
    log(`${label} queued behind the current native build.`);
  }
  queue.enqueue(target);
}

/** Kills a process and everything under it: children first, then KILL any survivors. */
async function killTree(pid) {
  const ps = await capture('ps', ['-axo', 'pid=,ppid=']).catch(() => ({ stdout: '' }));
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

function stopNative(target, { restart = false } = {}) {
  const entry = natives.get(target);
  if (!entry) return false;
  entry.restart = restart;
  killTree(entry.child.pid);
  return true;
}

function openWeb() {
  const opener = spawn('open', [URL], { stdio: 'ignore' });
  opener.on('error', () => log(`Could not open a browser. Open ${URL} yourself.`));
}

function handleAction(action) {
  switch (action) {
    case 'web':
      openWeb();
      break;
    case 'mac':
    case 'android':
      requestLaunch(action);
      break;
    case 'relaunch': {
      if (natives.size === 0) {
        log('Nothing is running to relaunch.');
        break;
      }
      for (const target of [...natives.keys()]) {
        stopNative(target, { restart: true });
      }
      break;
    }
    case 'stop': {
      const cancelled = queue.cancelPending();
      if (natives.size === 0 && cancelled.length === 0) {
        log('Nothing native is running.');
        break;
      }
      for (const target of [...natives.keys()]) {
        stopNative(target);
      }
      for (const target of cancelled) {
        log(`${LABELS[target]}: cancelled before it started.`);
      }
      break;
    }
    case 'help':
      log(formatMenu(menuState()));
      break;
    case 'quit':
      quit(0);
      break;
    default:
      break;
  }
}

function onKeys(chunk) {
  for (const key of chunk) {
    const action = keyToAction(key);
    if (action) handleAction(action);
  }
}

function restoreTerminal() {
  if (keysAvailable && process.stdin.isRaw) {
    process.stdin.setRawMode(false);
  }
}

/** Stops every native app and Vite, then exits. Safe to call more than once. */
async function quit(code) {
  if (shuttingDown) return;
  shuttingDown = true;
  restoreTerminal();
  queue.cancelPending();
  const trees = [...natives.values()].map((entry) => killTree(entry.child.pid));
  if (vite) trees.push(killTree(vite.pid));
  await Promise.all(trees);
  await removeAndroidReverse();
  log('Stopped.');
  process.exit(code);
}

function waitForPort(port, timeoutMs) {
  const deadline = Date.now() + timeoutMs;
  return new Promise((resolve) => {
    const attempt = () => {
      if (shuttingDown) return resolve(false);
      const socket = net.connect({ host: 'localhost', port });
      socket.once('connect', () => {
        socket.destroy();
        resolve(true);
      });
      socket.once('error', () => {
        socket.destroy();
        if (Date.now() > deadline) return resolve(false);
        setTimeout(attempt, 250);
      });
    };
    attempt();
  });
}

async function main() {
  if (!keysAvailable) {
    log(formatMenu(menuState()));
  }

  vite = spawn('pnpm', ['dev'], { stdio: ['ignore', 'inherit', 'inherit'] });
  vite.on('exit', (code) => {
    if (!shuttingDown) {
      log(`Vite stopped (exit ${code ?? 'signal'}).`);
      quit(code ?? 1);
    }
  });

  const ready = await waitForPort(DEV_PORT, PORT_WAIT_MS);
  if (!ready || shuttingDown) {
    if (!ready && !shuttingDown) {
      log(`Vite did not start on port ${DEV_PORT} within a minute.`);
      quit(1);
    }
    return;
  }
  log(`Vite is up at ${URL}`);

  if (keysAvailable) {
    process.stdin.setRawMode(true);
    process.stdin.setEncoding('utf8');
    process.stdin.on('data', onKeys);
    process.stdin.resume();
  }

  if (options.web) openWeb();
  if (options.mac) requestLaunch('mac');
  if (options.android) requestLaunch('android');
  if (keysAvailable) {
    log(formatMenu(menuState()));
  }
}

process.on('SIGINT', () => quit(130));
process.on('SIGTERM', () => quit(143));
process.on('SIGHUP', () => quit(129));
process.on('exit', restoreTerminal);

main().catch((error) => {
  log(`dev: ${error.message}`);
  quit(1);
});
