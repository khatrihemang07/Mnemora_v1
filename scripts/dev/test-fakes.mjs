// Test doubles for the controller's I/O. Nothing here starts a process: spawns, adb runs
// and kills are recorded, and each adb run either answers at once or waits for the test.

const PHONE_ONLINE = 'List of devices attached\nPHONE\tdevice\n';

/** A clock the test moves by hand; timers fire when `advance` passes their time. */
export function fakeTimers() {
  let now = 0;
  let nextId = 1;
  const pending = new Map();
  return {
    setTimeout(fn, ms) {
      const id = nextId++;
      pending.set(id, { at: now + ms, fn });
      return id;
    },
    clearTimeout(id) {
      pending.delete(id);
    },
    advance(ms) {
      now += ms;
      for (const [id, timer] of [...pending]) {
        if (timer.at <= now) {
          pending.delete(id);
          timer.fn();
        }
      }
    },
    get pendingCount() {
      return pending.size;
    },
  };
}

/**
 * Deps for createController. `autoAdb` answers adb calls at once with a ready phone;
 * set it to false to hold them, then finish a run by hand with `runs[i].finish(result)`.
 */
export function fakeDeps({ autoAdb = true } = {}) {
  const spawns = [];
  const runs = [];
  const killed = [];
  const opened = [];
  const state = { autoAdb };

  const deps = {
    spawnNative(spec, handlers) {
      const child = { ...spec, handlers, pid: 5000 + spawns.length };
      spawns.push(child);
      return { pid: child.pid };
    },
    run(command, args) {
      let resolve;
      const done = new Promise((r) => {
        resolve = r;
      });
      const run = {
        command,
        args,
        cancelled: false,
        finish(result) {
          resolve({ code: 0, stdout: '', stderr: '', ...result });
        },
        cancel() {
          // Like a real kill, the run only settles when the process reports back.
          run.cancelled = true;
        },
      };
      runs.push(run);
      if (state.autoAdb && command === 'adb') {
        const answer = args[0] === 'devices' ? { stdout: PHONE_ONLINE } : {};
        queueMicrotask(() => run.finish(answer));
      }
      return { done, cancel: () => run.cancel() };
    },
    killTree(pid) {
      killed.push(pid);
      return Promise.resolve();
    },
    openUrl(url) {
      opened.push(url);
    },
  };

  return { deps, spawns, runs, killed, opened, state };
}

/** Lets pending promise callbacks run. */
export function flush() {
  return new Promise((resolve) => setImmediate(resolve));
}
