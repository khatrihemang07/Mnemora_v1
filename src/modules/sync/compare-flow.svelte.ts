import type { ClashChoice } from '../../lib/ipc/bindings';

/**
 * Where a person is in Edit merged for one conflict: the step, the choice for each clash and the
 * text being polished. It is kept outside the page because the shell mounts a different layout across
 * the phone breakpoint, which remounts the page; resizing must not send the person back to Compare.
 */
export class CompareFlow {
  step = $state<'compare' | 'choose' | 'polish'>('compare');
  choices = $state<ClashChoice[]>([]);
  draft = $state('');
  showOriginal = $state(false);
}

const flows = new Map<string, CompareFlow>();

/** The flow of conflict `id`, made on first use. */
export function compareFlow(id: string): CompareFlow {
  let flow = flows.get(id);
  if (!flow) {
    flow = new CompareFlow();
    flows.set(id, flow);
  }
  return flow;
}

/** Drops the flow once the conflict is settled, so a later visit starts at Compare. */
export function forgetCompareFlow(id: string) {
  flows.delete(id);
}
