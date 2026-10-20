/** Pointer kinds that never hover: a tap must not leave a tooltip behind. */
const NO_HOVER = new Set(['touch', 'pen']);

let lastPointerType: string | null = null;
let tracking = false;

/** Starts remembering the kind of the most recent pointer (once; call from the Tooltip). */
export function trackPointerType(): void {
  if (tracking || typeof window === 'undefined') return;
  tracking = true;
  window.addEventListener(
    'pointerdown',
    (e) => {
      lastPointerType = e.pointerType;
    },
    { capture: true, passive: true },
  );
}

/** Whether a tooltip may open after a pointer of this kind (null: keyboard or unknown). */
export function tooltipAllowed(pointerType: string | null): boolean {
  return pointerType === null || !NO_HOVER.has(pointerType);
}

export const tooltipAllowedNow = () => tooltipAllowed(lastPointerType);

/** The shortcut hint to show: none on a phone, where there is no keyboard to use it. */
export function shownShortcut(shortcut: string | undefined, phone: boolean): string | undefined {
  return phone ? undefined : shortcut;
}
