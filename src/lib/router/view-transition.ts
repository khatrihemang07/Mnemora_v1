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
  root.dataset.viewTransition = '';
  let result!: T;
  const transition = doc.startViewTransition(async () => {
    result = await update();
    await tick(); // the new route must be in the DOM before the browser snapshots it
  });
  try {
    await transition.finished;
  } finally {
    delete root.dataset.viewTransition;
  }
  return result;
}
