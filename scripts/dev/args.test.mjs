import { describe, expect, it } from 'vitest';
import { parseArgs } from './args.mjs';

describe('parseArgs', () => {
  it('defaults to no native launch and Production', () => {
    expect(parseArgs([])).toEqual({ sandbox: false, web: false, launch: [] });
  });

  it('reads each target flag as a launch, in the order given, without duplicates', () => {
    expect(parseArgs(['--android', '--mac', '--android']).launch).toEqual(['android', 'mac']);
  });

  it('reads --web and --sandbox', () => {
    expect(parseArgs(['--web', '--sandbox'])).toEqual({ sandbox: true, web: true, launch: [] });
  });

  it('rejects an unknown flag so a typo never silently does nothing', () => {
    expect(() => parseArgs(['--macc'])).toThrow(/Unknown option: --macc/);
    expect(() => parseArgs(['--macc'])).toThrow(/Usage: /);
  });

  it('accepts a target flag for every registered target', () => {
    expect(parseArgs(['--mac', '--android']).launch).toEqual(['mac', 'android']);
  });
});
