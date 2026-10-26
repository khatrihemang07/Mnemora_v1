/**
 * How many bottom sheets are open on a phone. The toast host reads it: toasts move to the top of
 * the screen while a sheet covers the bottom, so they never sit on the sheet's content.
 */
export const openSheets = $state({ count: 0 });

/** Whether Todo's + button is on screen on a phone: toasts then sit above it instead of over it. */
export const phoneFab = $state({ shown: false });

/**
 * On a phone, the height in pixels a page's own bottom (the AI composer, with the tab bar under it)
 * takes up. Toasts sit above it while it is shown; 0 when the page has none (the Toaster then uses
 * `phoneFab`).
 */
export const phoneBottom = $state({ px: 0 });
