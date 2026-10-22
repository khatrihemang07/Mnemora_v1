import { describe, expect, it, vi } from 'vitest';
import { resumeWhenBack } from './resume';

function sources() {
  const doc = Object.assign(new EventTarget(), { hidden: false });
  const win = new EventTarget();
  return { doc, win };
}

describe('resumeWhenBack', () => {
  it('resumes when the app becomes visible, not when it is hidden', () => {
    const s = sources();
    const resume = vi.fn();
    resumeWhenBack(resume, s);
    s.doc.hidden = true;
    s.doc.dispatchEvent(new Event('visibilitychange'));
    expect(resume).not.toHaveBeenCalled();
    s.doc.hidden = false;
    s.doc.dispatchEvent(new Event('visibilitychange'));
    expect(resume).toHaveBeenCalledTimes(1);
  });

  it('resumes when the network comes back', () => {
    const s = sources();
    const resume = vi.fn();
    resumeWhenBack(resume, s);
    s.win.dispatchEvent(new Event('online'));
    expect(resume).toHaveBeenCalledTimes(1);
  });

  it('stops listening when told to', () => {
    const s = sources();
    const resume = vi.fn();
    resumeWhenBack(resume, s)();
    s.win.dispatchEvent(new Event('online'));
    s.doc.dispatchEvent(new Event('visibilitychange'));
    expect(resume).not.toHaveBeenCalled();
  });
});
