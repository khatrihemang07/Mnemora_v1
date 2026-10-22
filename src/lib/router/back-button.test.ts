import { describe, expect, it, vi } from 'vitest';
import { handleBack, interceptBack } from './back-button.svelte';

const setup = (bridge: { moveTaskToBack?: () => void } | undefined) => ({
  goBack: vi.fn(),
  exitApp: vi.fn(),
  bridge,
});

describe('handleBack', () => {
  it('lets an open editor take Back', () => {
    const handler = vi.fn();
    const off = interceptBack(handler);
    const d = setup({ moveTaskToBack: vi.fn() });
    handleBack({ canGoBack: true }, d);
    expect(handler).toHaveBeenCalledOnce();
    expect(d.goBack).not.toHaveBeenCalled();
    off();
  });

  it('goes back in history when it can', () => {
    const d = setup({ moveTaskToBack: vi.fn() });
    handleBack({ canGoBack: true }, d);
    expect(d.goBack).toHaveBeenCalledOnce();
    expect(d.exitApp).not.toHaveBeenCalled();
  });

  it('moves the task to the back at the root, without exiting the process', () => {
    const d = setup({ moveTaskToBack: vi.fn() });
    handleBack({ canGoBack: false }, d);
    expect(d.bridge?.moveTaskToBack).toHaveBeenCalledOnce();
    expect(d.exitApp).not.toHaveBeenCalled();
  });

  it('falls back to exiting when the native bridge is missing', () => {
    const d = setup(undefined);
    handleBack({ canGoBack: false }, d);
    expect(d.exitApp).toHaveBeenCalledOnce();
  });
});
