import { describe, expect, it } from 'vitest';
import { pageHeader } from './page-header.svelte';

describe('pageHeader', () => {
  it('shows the latest page and clears on release', () => {
    const release = pageHeader.set({ title: 'Home' });
    expect(pageHeader.current?.title).toBe('Home');
    release();
    expect(pageHeader.current).toBeNull();
  });

  it('keeps a newer page when an older one releases late', () => {
    const releaseHome = pageHeader.set({ title: 'Home' });
    const releaseSettings = pageHeader.set({ title: 'Settings' });
    releaseHome();
    expect(pageHeader.current?.title).toBe('Settings');
    releaseSettings();
    expect(pageHeader.current).toBeNull();
  });
});

describe('pageHeader.env', () => {
  it('starts unknown and holds what the shell sets', () => {
    expect(pageHeader.env).toBeNull();
    pageHeader.env = 'sandbox';
    expect(pageHeader.env).toBe('sandbox');
    pageHeader.env = null;
  });
});
