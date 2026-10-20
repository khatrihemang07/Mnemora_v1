/** Which Settings section is open. Module-level so it survives leaving and returning. */
export const settingsNav = $state<{ section: string | null }>({ section: null });
