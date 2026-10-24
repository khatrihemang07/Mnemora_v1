// Finds the processes under a pid from `ps -axo pid=,ppid=` output. Pure.

/** Pids below `rootPid`, children before parents, never the root itself. */
export function descendantPids(psText, rootPid) {
  const children = new Map();
  for (const line of psText.split('\n')) {
    const [pid, ppid] = line.trim().split(/\s+/).map(Number);
    if (!Number.isInteger(pid) || !Number.isInteger(ppid)) continue;
    if (!children.has(ppid)) children.set(ppid, []);
    children.get(ppid).push(pid);
  }
  const out = [];
  const visit = (pid) => {
    for (const child of children.get(pid) ?? []) {
      visit(child);
      out.push(child);
    }
  };
  visit(rootPid);
  return out;
}
