import { describe, expect, it } from 'vitest';
import { portInUseMessage } from './port.mjs';

describe('portInUseMessage', () => {
  it('names the port and says what to do, the same way the Makefile did', () => {
    expect(portInUseMessage(47821)).toBe(
      "Port 47821 is already in use: a dev server is probably running (see 'pgrep -fl vite'). Stop it first.",
    );
  });
});
