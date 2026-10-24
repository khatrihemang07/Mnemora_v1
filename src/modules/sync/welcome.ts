/**
 * Whether the Devices tab is the welcome card: nothing is paired (story 6). Two lists say so, the
 * Sync state's count and the Devices list, and either may notice first (after Leave or Forget).
 */
export function showWelcome(paired: number | undefined, listed: number | undefined): boolean {
  return paired === 0 || listed === 0;
}
