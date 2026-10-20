/** Display name wins; else the OS first name; else no name at all. */
export const homeName = (displayName: string, osFirstName: string | undefined): string =>
  displayName.trim() || osFirstName?.trim() || '';
