// Starts, queues, restarts and stops the native apps. Every effect goes through `deps`
// (spawnNative, run, killTree, openUrl), so the tests drive it with fakes and start nothing.
//
// A session is one native app from its preparation until its process ends. Its build
// slot is held from preparation until the target's end-of-build marker (see registry),
// or until BUILD_SLOT_TIMEOUT_MS if no marker ever prints; a running app never holds it.

import { BUILD_SLOT_TIMEOUT_MS, DEV_URL } from './config.mjs';
import { ACTION } from './keys.mjs';
import { formatMenu, formatStatus } from './menu.mjs';
import { createBuildQueue } from './queue.mjs';
import { environmentName, TARGETS } from './targets.mjs';

const CANCELLED = 'cancelled before it started';

export function createController({ sandbox, keysAvailable, deps, log, timers = globalThis }) {
  const environment = environmentName(sandbox);
  const sessions = new Map(); // target id -> session, from preparation until its process ends
  let shuttingDown = false;
  const queue = createBuildQueue(startSession);

  const labelOf = (id) => TARGETS[id].label;
  const labels = (list) => list.map(labelOf).join(', ');
  const environmentOf = (id) => environmentName(TARGETS[id].usesSandbox(sandbox));
  // Only the targets that run on this session's Environment: `make dev` never offers Android.
  const ids = Object.keys(TARGETS).filter((id) => environmentOf(id) === environment);
  const isCancelled = (session) => shuttingDown || session.cancelled;

  function states() {
    const queued = queue.pendingTargets();
    return Object.fromEntries(
      ids.map((id) => {
        const session = sessions.get(id);
        if (session) return [id, session.phase];
        return [id, queued.includes(id) ? 'queued' : 'stopped'];
      }),
    );
  }

  function announce() {
    log(formatStatus({ environment, states: states(), queue: queue.pendingTargets() }));
  }

  function menu() {
    return formatMenu({
      environment,
      states: states(),
      environments: Object.fromEntries(ids.map((id) => [id, environmentOf(id)])),
      queue: queue.pendingTargets(),
      keysAvailable,
    });
  }

  /** Frees the build slot once. Safe to call from any path that ends the build phase. */
  function freeSlot(session) {
    timers.clearTimeout(session.timer);
    if (session.slotFreed) return;
    session.slotFreed = true;
    session.slot.release();
  }

  /** The queue calls this when the slot is granted to a target. */
  function startSession(slot) {
    const session = {
      target: slot.target,
      slot,
      phase: 'preparing',
      restart: false,
      cancelled: false,
      ended: false,
      slotFreed: false,
      child: null,
      runs: new Set(),
      undo: null,
      timer: null,
      preparing: null,
    };
    sessions.set(session.target, session);
    session.timer = timers.setTimeout(() => timeOut(session), BUILD_SLOT_TIMEOUT_MS);
    announce();
    session.preparing = prepareAndSpawn(session);
  }

  function timeOut(session) {
    if (session.ended || session.slotFreed) return;
    const minutes = BUILD_SLOT_TIMEOUT_MS / 60_000;
    log(
      `${labelOf(session.target)}: no build marker after ${minutes} min; the next build may start.`,
    );
    freeSlot(session);
    announce();
  }

  /** `run` for a preparation step: refuses to start a command once the session is cancelled. */
  function runFor(session) {
    return (command, args) => {
      if (isCancelled(session)) return Promise.reject(new Error(CANCELLED));
      const handle = deps.run(command, args);
      session.runs.add(handle);
      return handle.done.finally(() => session.runs.delete(handle));
    };
  }

  function cancelRuns(session) {
    for (const handle of session.runs) handle.cancel();
  }

  async function prepareAndSpawn(session) {
    const plan = TARGETS[session.target];
    try {
      if (plan.prepare) {
        session.undo = await plan.prepare(runFor(session));
      }
    } catch (error) {
      if (isCancelled(session)) {
        endSession(session, CANCELLED);
        return;
      }
      log(`${plan.label}: ${error.message}`);
      endSession(session, null);
      return;
    }
    // Checked after every await: a quit or a restart during preparation must not spawn cargo.
    if (isCancelled(session)) {
      endSession(session, CANCELLED);
      return;
    }
    spawnBuild(session, plan);
  }

  function spawnBuild(session, plan) {
    try {
      // Synchronous, so the handle is recorded before anything else can run.
      session.child = deps.spawnNative(plan.command(plan.usesSandbox(sandbox)), {
        onLine: (line) => onLine(session, line),
        onExit: (code, signal, error) =>
          endSession(session, error ?? (signal ? `signal ${signal}` : `exit ${code}`)),
      });
    } catch (error) {
      log(`${plan.label}: ${error.message}`);
      endSession(session, null);
      return;
    }
    session.phase = 'building';
    log(`${plan.label}: building (${environmentOf(session.target)})`);
    announce();
  }

  function onLine(session, line) {
    log(`[${session.target}] ${line}`);
    if (
      !session.ended &&
      session.phase === 'building' &&
      TARGETS[session.target].endsBuild.test(line)
    ) {
      session.phase = 'running';
      freeSlot(session);
      announce();
    }
  }

  /** Ends a session once; later calls do nothing. Returns the undo's promise, if any. */
  function endSession(session, note) {
    if (session.ended) return Promise.resolve();
    session.ended = true;
    if (sessions.get(session.target) === session) sessions.delete(session.target);
    freeSlot(session);
    if (note) log(`${labelOf(session.target)}: stopped (${note})`);
    const undo = runUndo(session);
    if (session.restart && !shuttingDown) queue.enqueue(session.target);
    announce();
    return undo;
  }

  /** Undoes what preparation set up (the adb reverse). Its own run, so cancellation does not stop it. */
  function runUndo(session) {
    const undo = session.undo;
    session.undo = null;
    if (!undo) return Promise.resolve();
    return deps.run(undo.command, undo.args).done.catch(() => {});
  }

  function stopSession(session, { restart }) {
    session.restart = restart;
    if (session.child) {
      deps.killTree(session.child.pid);
      return;
    }
    session.cancelled = true;
    cancelRuns(session);
  }

  function launch(target) {
    if (shuttingDown) return;
    const label = labelOf(target);
    if (!ids.includes(target)) {
      log(
        `${label} runs on ${environmentOf(target)} data only. Use make ${sandbox ? 'dev' : 'sandbox'}.`,
      );
      return;
    }
    const session = sessions.get(target);
    if (session) {
      log(`${label} is ${session.phase}. r relaunches it.`);
      return;
    }
    if (queue.pendingTargets().includes(target)) {
      log(`${label} is already queued.`);
      return;
    }
    queue.enqueue(target);
    if (queue.pendingTargets().includes(target)) {
      log(`${label} is queued behind the current native build.`);
    }
  }

  function relaunch() {
    if (shuttingDown) return;
    const active = [...sessions.values()];
    const waiting = queue.pendingTargets();
    if (active.length === 0 && waiting.length === 0) {
      log('Nothing is running to relaunch.');
      return;
    }
    for (const session of active) stopSession(session, { restart: true });
    // A waiting target is cancelled and queued again at the back.
    for (const target of waiting) {
      queue.remove(target);
      queue.enqueue(target);
    }
    log(`Relaunching ${labels([...active.map((s) => s.target), ...waiting])}.`);
  }

  function stop() {
    if (shuttingDown) return;
    const active = [...sessions.values()];
    const waiting = queue.pendingTargets();
    if (active.length === 0) {
      log(
        waiting.length > 0
          ? `Nothing is running. Still queued: ${labels(waiting)}.`
          : 'Nothing native is running.',
      );
      return;
    }
    for (const session of active) stopSession(session, { restart: false });
    const still = waiting.length > 0 ? ` Still queued: ${labels(waiting)} (it starts next).` : '';
    log(`Stopping ${labels(active.map((s) => s.target))}.${still}`);
  }

  function stopForQuit(session) {
    session.restart = false;
    if (session.child) {
      return deps.killTree(session.child.pid).then(() => endSession(session, null));
    }
    session.cancelled = true;
    cancelRuns(session);
    return session.preparing.then(() => endSession(session, null));
  }

  /** Stops every native app and undoes Android's port forward. Launches are refused from now on. */
  async function shutdown() {
    shuttingDown = true;
    queue.cancelPending();
    await Promise.all([...sessions.values()].map(stopForQuit));
  }

  /** Runs one menu action. Quit is not here: the entry point owns the processes it stops. */
  function dispatch(action) {
    switch (action.action) {
      case ACTION.WEB:
        deps.openUrl(DEV_URL);
        return;
      case ACTION.LAUNCH:
        launch(action.target);
        return;
      case ACTION.RELAUNCH:
        relaunch();
        return;
      case ACTION.STOP:
        stop();
        return;
      case ACTION.HELP:
        log(menu());
        return;
      default:
    }
  }

  return { dispatch, shutdown, menu };
}
