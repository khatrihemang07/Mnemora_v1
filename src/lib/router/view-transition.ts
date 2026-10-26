import { tick } from 'svelte';

type VTDocument = Document & {
  startViewTransition?: (update: () => Promise<void>) => { finished: Promise<void> };
};

const reducedMotion = () =>
  document.documentElement.dataset.reduceMotion === 'true' ||
  window.matchMedia('(prefers-reduced-motion: reduce)').matches;

/**
 * Runs a route change inside a View Transition (N14-B fade, styled in app.css) when the
 * browser supports it and motion is allowed; otherwise just runs it and the keyframe fade on
 * the route container plays. `data-view-transition` on <html> turns that keyframe fade off
 * for the duration, so the page never fades twice.
 */
export async function withViewTransition<T>(update: () => Promise<T>): Promise<T> {
  const doc = document as VTDocument;
  if (!doc.startViewTransition || reducedMotion()) return update();
  const root = document.documentElement;
  // The newest transition owns the marker: an older one ending must not clear it under the new one.
  const turn = ++latestTurn;
  root.dataset.viewTransition = '';
  let result!: T;
  const transition = doc.startViewTransition(async () => {
    result = await update();
    await tick(); // the new route must be in the DOM before the browser snapshots it
  });
  try {
    await transition.finished;
  } catch (error) {
    // A newer navigation superseded this one. Its update still ran, so the route change is done.
    if (!isSuperseded(error)) throw error;
  } finally {
    if (turn === latestTurn) delete root.dataset.viewTransition;
  }
  return result;
}

let latestTurn = 0;

function isSuperseded(error: unknown): boolean {
  return error instanceof DOMException && error.name === 'AbortError';
}
