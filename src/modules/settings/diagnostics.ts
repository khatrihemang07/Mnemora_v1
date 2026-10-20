export interface DiagnosticsInput {
  version: string;
  environment: string;
  os: string;
  userAgent: string;
  errors: { at: string; message: string }[];
}

/** Plain text a user can paste into a bug report. Contains no Settings values or paths. */
export function buildDiagnostics(d: DiagnosticsInput): string {
  const errors = d.errors.length
    ? d.errors.map((e) => `  ${e.at} ${e.message}`).join('\n')
    : '  none';
  return [
    `Mnemora ${d.version} (${d.environment})`,
    `OS: ${d.os}`,
    `User agent: ${d.userAgent}`,
    'Recent UI errors:',
    errors,
  ].join('\n');
}
