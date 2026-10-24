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
  /** The choices `draft` was made from: the draft is only made again when they change. */
  draftFor = $state<string | null>(null);
}

/** Whether the text in the Polish step is still the person's own for these choices (Back and Next keep it). */
export function draftIsCurrent(flow: CompareFlow): boolean {
  return flow.draft !== '' && flow.draftFor === flow.choices.join(',');
}

/** Cancel (or a conflict that is no longer open): the choices and the text are thrown away. */
export function discardEdit(flow: CompareFlow) {
  flow.step = 'compare';
  flow.choices = [];
  flow.draft = '';
  flow.draftFor = null;
}

/** Esc in the Choose or Polish step is Cancel (T4); Compare is a page, so Esc leaves it alone. True when it cancelled. */
export function escapeCancels(
  flow: CompareFlow,
  e: { key: string; defaultPrevented: boolean },
): boolean {
  if (e.key !== 'Escape' || e.defaultPrevented || flow.step === 'compare') return false;
  discardEdit(flow);
  return true;
}

/**
 * Back inside Edit merged: Polish goes back to Choose, Choose to Compare; the choices and the text stay,
 * so nothing typed is lost (R4). Compare itself has no step to go back to (`null`: Back leaves the page).
 */
export function stepBack(step: CompareFlow['step']): CompareFlow['step'] | null {
  return step === 'polish' ? 'choose' : step === 'choose' ? 'compare' : null;
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
