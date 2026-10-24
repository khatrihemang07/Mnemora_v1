// Reads `adb devices` and picks the one phone to use. Pure.

const DEVICE_LINE = /^(\S+)\s+(\S+)$/;

/** Every device adb lists, with its state (device, unauthorized, offline, ...). */
export function parseAdbDevices(text) {
  const devices = [];
  for (const line of text.split('\n')) {
    const match = DEVICE_LINE.exec(line.trim());
    if (!match || match[1] === 'List' || match[1] === '*') continue;
    devices.push({ serial: match[1], state: match[2] });
  }
  return devices;
}

/**
 * The serial of the one phone that is ready. Every other outcome is an error with a message
 * that says what to do: a phone that is listed but not ready is not "No phone found".
 */
export function pickPhone(devices) {
  const ready = devices.filter((device) => device.state === 'device');
  if (ready.length === 1) return ready[0].serial;
  if (ready.length > 1) {
    throw new Error('More than one phone is connected. Leave one connected, then press a.');
  }
  if (devices.some((device) => device.state === 'unauthorized')) {
    throw new Error(
      'The phone is not authorised for USB debugging. Unlock it, tap Allow on the USB debugging prompt, then press a.',
    );
  }
  if (devices.some((device) => device.state === 'offline')) {
    throw new Error(
      'The phone is offline to adb. Reconnect the USB cable (or toggle USB debugging), then press a.',
    );
  }
  const other = devices[0];
  if (other) {
    throw new Error(`The phone reports adb state "${other.state}". Reconnect it, then press a.`);
  }
  throw new Error('No phone found. Connect it over USB with USB debugging on, then press a.');
}
