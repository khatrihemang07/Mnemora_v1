/** Pointer kinds that never hover: a tap must not leave a tooltip behind. */
const NO_HOVER = new Set(['touch', 'pen']);

let lastPointerType: string | null = null;
let lastInputWasKeyboard = false;
let tracking = false;

/** Starts remembering the kind of the most recent input (once; call from the Tooltip). */
export function trackPointerType(): void {
  if (tracking || typeof window === 'undefined') return;
  tracking = true;
  window.addEventListener(
    'pointerdown',
    (e) => {
      lastPointerType = e.pointerType;
      lastInputWasKeyboard = false;
    },
    { capture: true, passive: true },
  );
  window.addEventListener(
    'keydown',
    () => {
      lastInputWasKeyboard = true;
    },
    { capture: true, passive: true },
  );
}

/** Whether a tooltip may open after a pointer of this kind (null: keyboard or unknown). */
export function tooltipAllowed(pointerType: string | null): boolean {
  return pointerType === null || !NO_HOVER.has(pointerType);
}

export const tooltipAllowedNow = () => tooltipAllowed(lastPointerType);

/**
 * Whether an open caused by focus may show: only keyboard focus (`:focus-visible` after a key
 * press). Focus restored by code, e.g. a Menu or Sheet closing, or a click, must not.
 */
export function focusOpenAllowed(focusVisible: boolean, lastInputKeyboard: boolean): boolean {
  return focusVisible && lastInputKeyboard;
}

/** Whether the trigger's current focus should open a tooltip (DOM wrapper over the pure rule). */
export function focusOpenAllowedFor(trigger: Element | null): boolean {
  let focusVisible = false;
  try {
    focusVisible = !!trigger?.matches(':focus-visible');
  } catch {
    // Old engines without :focus-visible: stay closed.
  }
  return focusOpenAllowed(focusVisible, lastInputWasKeyboard);
}

/** The shortcut hint to show: none on a phone, where there is no keyboard to use it. */
export function shownShortcut(shortcut: string | undefined, phone: boolean): string | undefined {
  return phone ? undefined : shortcut;
}
