import { describe, expect, it } from 'vitest';
import { parseAdbDevices, pickPhone } from './adb.mjs';

describe('parseAdbDevices', () => {
  it('lists every attached device with its state', () => {
    const text = [
      'List of devices attached',
      '10BD8H0UM50004X\tdevice',
      'emulator-5554\toffline',
      'ABC123\tunauthorized',
      '',
    ].join('\n');
    expect(parseAdbDevices(text)).toEqual([
      { serial: '10BD8H0UM50004X', state: 'device' },
      { serial: 'emulator-5554', state: 'offline' },
      { serial: 'ABC123', state: 'unauthorized' },
    ]);
  });

  it('skips the daemon start-up lines adb prints before the list', () => {
    const text =
      '* daemon not running; starting now\n* daemon started successfully\nList of devices attached\n';
    expect(parseAdbDevices(text)).toEqual([]);
  });
});

describe('pickPhone', () => {
  it('returns the serial of the one phone that is ready', () => {
    expect(pickPhone([{ serial: 'PHONE', state: 'device' }])).toBe('PHONE');
  });

  it('says no phone when adb lists none', () => {
    expect(() => pickPhone([])).toThrow(/No phone found/);
  });

  it('says the phone is not authorised for USB debugging when it is unauthorized', () => {
    expect(() => pickPhone([{ serial: 'PHONE', state: 'unauthorized' }])).toThrow(
      /not authorised for USB debugging/,
    );
  });

  it('says the phone is offline when it is offline, and does not say No phone found', () => {
    let message = '';
    try {
      pickPhone([{ serial: 'PHONE', state: 'offline' }]);
    } catch (error) {
      message = error.message;
    }
    expect(message).toMatch(/offline/);
    expect(message).not.toMatch(/No phone found/);
  });

  it('names any other adb state rather than calling it missing', () => {
    expect(() => pickPhone([{ serial: 'PHONE', state: 'recovery' }])).toThrow(/"recovery"/);
  });

  it('refuses when more than one phone is ready', () => {
    expect(() =>
      pickPhone([
        { serial: 'A', state: 'device' },
        { serial: 'B', state: 'device' },
      ]),
    ).toThrow(/More than one phone/);
  });

  it('uses the one ready phone even when another is unauthorized', () => {
    expect(
      pickPhone([
        { serial: 'A', state: 'device' },
        { serial: 'B', state: 'unauthorized' },
      ]),
    ).toBe('A');
  });
});
