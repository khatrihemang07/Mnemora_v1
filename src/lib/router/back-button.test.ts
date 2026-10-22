import { afterEach, describe, expect, it, vi } from 'vitest';
import { handleBack, interceptBack } from './back-button.svelte';

const makeDeps = () => ({ goBack: vi.fn(), leave: vi.fn() });

describe('handleBack', () => {
  let off: (() => void) | undefined;
  afterEach(() => {
    off?.();
    off = undefined;
  });

  it('lets an open editor take Back', () => {
    const handler = vi.fn();
    off = interceptBack(handler);
    const deps = makeDeps();
    handleBack({ canGoBack: true }, deps);
    expect(handler).toHaveBeenCalledOnce();
    expect(deps.goBack).not.toHaveBeenCalled();
    expect(deps.leave).not.toHaveBeenCalled();
  });

  it('goes back in history when it can', () => {
    const deps = makeDeps();
    handleBack({ canGoBack: true }, deps);
    expect(deps.goBack).toHaveBeenCalledOnce();
    expect(deps.leave).not.toHaveBeenCalled();
  });

  it('leaves the app at the root', () => {
    const deps = makeDeps();
    handleBack({ canGoBack: false }, deps);
    expect(deps.leave).toHaveBeenCalledOnce();
    expect(deps.goBack).not.toHaveBeenCalled();
  });
});
