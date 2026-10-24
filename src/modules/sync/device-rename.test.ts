import { describe, expect, it } from 'vitest';
import { DeviceRename } from './device-rename.svelte';

const refuse = (message: string) => async () => {
  throw new Error(message);
};

describe('renaming a device', () => {
  it('closes the field and keeps nothing stale after a good save', async () => {
    const saved: string[] = [];
    const r = new DeviceRename(async (n) => void saved.push(n));
    r.start('Pixel 8');
    r.type('Phone');
    await r.commit('Pixel 8');
    expect(saved).toEqual(['Phone']);
    expect(r.editing).toBe(false);
    expect(r.error).toBeNull();
  });

  it('keeps the field open with the reason when refused', async () => {
    const r = new DeviceRename(refuse('Another of your devices already has that name'));
    r.start('Pixel 8');
    r.type('Air');
    await r.commit('Pixel 8');
    expect(r.editing).toBe(true);
    expect(r.error).toBe('Another of your devices already has that name');
  });

  it('Esc after a refusal drops the text and the message, so the saved name shows again (A4)', async () => {
    const r = new DeviceRename(refuse('Names are 1 to 40 characters'));
    r.start('Alex’s Mac');
    r.type('');
    await r.commit('Alex’s Mac');
    expect(r.error).not.toBeNull();
    r.cancel();
    expect(r.editing).toBe(false);
    expect(r.error).toBeNull();
    // Typing again starts a fresh draft from the saved name.
    r.start('Alex’s Mac');
    expect(r.text).toBe('Alex’s Mac');
  });

  it('replaces the old message with the new one, and typing clears it', async () => {
    let message = 'Another of your devices already has that name';
    const r = new DeviceRename(async () => {
      throw new Error(message);
    });
    r.start('Pixel 8');
    r.type('Air');
    await r.commit('Pixel 8');
    message = 'Give this device a name';
    r.type('');
    expect(r.error).toBeNull();
    await r.commit('Pixel 8');
    expect(r.error).toBe('Give this device a name');
  });

  it('does nothing for the same name and saves once for Enter plus blur', async () => {
    let calls = 0;
    const r = new DeviceRename(async () => void calls++);
    r.start('Pixel 8');
    r.type(' Pixel 8 ');
    await r.commit('Pixel 8');
    expect(calls).toBe(0);
    expect(r.editing).toBe(false);
    r.start('Pixel 8');
    r.type('Phone');
    await Promise.all([r.commit('Pixel 8'), r.commit('Pixel 8')]);
    expect(calls).toBe(1);
  });
});
