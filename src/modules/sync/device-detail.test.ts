import { describe, expect, it } from 'vitest';
import type { DeviceDetail } from '../../lib/ipc/bindings';
import { addressLines, pairedText, versionsText } from './device-detail';

const NOW = new Date('2026-10-23T12:00:00Z');

describe('versionsText', () => {
  it('lists the app version, then the Modules in the sidebar order', () => {
    expect(
      versionsText({
        app_version: '0.9.2',
        data_versions: [
          { module: 'core', version: 4 },
          { module: 'todo', version: 9 },
          { module: 'journal', version: 3 },
        ],
      }),
    ).toBe('Mnemora 0.9.2 · Journal 3 · Todo 9 · Core 4');
  });

  it('puts a Module it does not know last and says nothing before the device was reached', () => {
    expect(
      versionsText({
        app_version: null,
        data_versions: [
          { module: 'zeta', version: 1 },
          { module: 'core', version: 4 },
        ],
      }),
    ).toBe('Core 4 · Zeta 1');
    expect(versionsText({ app_version: null, data_versions: [] })).toBeNull();
  });
});

describe('addressLines', () => {
  it('says when each address last worked', () => {
    const lines = addressLines(
      {
        addresses: [
          { address: '192.168.1.31:47311', last_ok_at: NOW.getTime() - 2 * 60_000 },
          { address: '100.88.12.4:47311', last_ok_at: NOW.getTime() - 3 * 86_400_000 },
          { address: '10.0.0.2:47311', last_ok_at: null },
        ],
      },
      NOW,
    );
    expect(lines).toEqual([
      { address: '192.168.1.31:47311', when: 'worked 2 min ago' },
      { address: '100.88.12.4:47311', when: 'worked 3 days ago' },
      { address: '10.0.0.2:47311', when: 'not tried yet' },
    ]);
  });
});

describe('pairedText', () => {
  const base: Pick<DeviceDetail, 'paired_at' | 'paired_by'> = {
    paired_at: Date.UTC(2026, 9, 14, 12),
    paired_by: 'Hemang’s MacBook Pro',
  };
  it('names the device that paired it', () => {
    expect(pairedText(base)).toMatch(/2026, by Hemang’s MacBook Pro$/);
  });
  it('leaves the "by" out when it is not known', () => {
    expect(pairedText({ ...base, paired_by: null })).toMatch(/2026$/);
  });
});
