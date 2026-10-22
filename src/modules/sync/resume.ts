/** The parts of `document` and `window` that say the app came back or the network changed. */
export interface ResumeSources {
  doc: {
    hidden: boolean;
    addEventListener(type: 'visibilitychange', listener: () => void): void;
    removeEventListener(type: 'visibilitychange', listener: () => void): void;
  };
  win: {
    addEventListener(type: 'online', listener: () => void): void;
    removeEventListener(type: 'online', listener: () => void): void;
  };
}

/**
 * Calls `resume` when the app comes back to the front or the network comes back, so devices that
 * could not be reached are tried again from the start of the retry schedule (5 s, 15 s, 60 s, ...)
 * instead of waiting out a long pause. Returns what stops listening.
 */
export function resumeWhenBack(
  resume: () => void,
  { doc, win }: ResumeSources = { doc: document, win: window },
): () => void {
  const onVisible = () => {
    if (!doc.hidden) resume();
  };
  doc.addEventListener('visibilitychange', onVisible);
  win.addEventListener('online', resume);
  return () => {
    doc.removeEventListener('visibilitychange', onVisible);
    win.removeEventListener('online', resume);
  };
}
