/** A light tick: long enough to feel on a phone, short enough to read as a click. */
export const TICK_MS = 12;

/**
 * A light haptic tick. There is no haptics plugin in the app, so this uses the Vibration API the
 * Android WebView offers (the manifest holds the VIBRATE permission); anywhere it is missing or
 * refused it does nothing.
 */
export function lightTick(
  nav: Pick<Navigator, 'vibrate'> | undefined = globalThis.navigator,
): void {
  try {
    nav?.vibrate?.(TICK_MS);
  } catch {
    // A haptic is never worth an error.
  }
}
