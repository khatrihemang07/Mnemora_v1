/**
 * How many bottom sheets are open on a phone. The toast host reads it: toasts move to the top of
 * the screen while a sheet covers the bottom, so they never sit on the sheet's content.
 */
export const openSheets = $state({ count: 0 });
