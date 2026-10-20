import { describe, expect, it } from 'vitest';
import { buildDiagnostics } from './diagnostics';

const base = { version: '0.1.0', environment: 'sandbox', os: 'macos', userAgent: 'UA', errors: [] };

describe('buildDiagnostics', () => {
  it('lists version, environment, OS and user agent', () => {
    const text = buildDiagnostics(base);
    expect(text).toContain('Mnemora 0.1.0 (sandbox)');
    expect(text).toContain('OS: macos');
    expect(text).toContain('User agent: UA');
    expect(text).toContain('  none');
  });
  it('includes recent errors', () => {
    const text = buildDiagnostics({
      ...base,
      errors: [{ at: '2026-10-20T10:00:00Z', message: 'boom' }],
    });
    expect(text).toContain('2026-10-20T10:00:00Z boom');
  });
});
