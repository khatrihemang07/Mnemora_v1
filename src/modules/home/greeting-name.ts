/** Display name wins; else the OS first name (already sanitised by Rust); else no name at all. */
export const homeName = (displayName: string, osFirstName: string | undefined): string =>
  displayName.trim() || (osFirstName?.trim() ?? '');
