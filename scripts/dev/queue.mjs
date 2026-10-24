// Runs one native build at a time. `start(slot)` gets a slot and must call `slot.release()`
// when its build phase is over; the next waiting target then starts. Pure: no timers here,
// the controller decides when a slot is freed.

export function createBuildQueue(start) {
  const pending = [];
  let slot = null;

  function pump() {
    while (!slot && pending.length > 0) {
      const current = { target: pending.shift(), released: false };
      current.release = () => {
        if (current.released) return;
        current.released = true;
        if (slot === current) slot = null;
        pump();
      };
      slot = current;
      start(current);
    }
  }

  return {
    /** Returns false when the target is already waiting. */
    enqueue(target) {
      if (pending.includes(target)) return false;
      pending.push(target);
      pump();
      return true;
    },
    /** Takes a waiting target out of the queue. Returns false when it was not waiting. */
    remove(target) {
      const index = pending.indexOf(target);
      if (index === -1) return false;
      pending.splice(index, 1);
      return true;
    },
    /** Drops every build that has not started; returns their targets. */
    cancelPending() {
      return pending.splice(0, pending.length);
    },
    pendingTargets() {
      return [...pending];
    },
  };
}
