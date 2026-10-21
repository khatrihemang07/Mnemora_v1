import { lightTick } from '../../lib/haptic';
import { LongPress } from '../../lib/long-press';
import { lockAxis, rowOffset, rowSwipeOutcome, type SwipeSides } from '../../lib/swipe';

/** Controls inside a row that keep their own taps: a touch that starts on one is not a gesture. */
const OWN_TAPS = 'button, a, input, textarea, [data-no-drag]';
/** The click a finished gesture would leave behind is swallowed for this long. */
const CLICK_GUARD_MS = 350;

export interface RowTouchOptions {
  /** Which sides can be swiped now (read at the start of each touch). */
  sides: () => SwipeSides;
  onright: () => void;
  onleft: () => void;
  /** The finger has rested long enough: pick the row up. `ontap` runs if it is lifted unmoved. */
  onhold: (e: PointerEvent) => void;
}

/**
 * What a finger does on a Task row on a phone: slide it sideways (right completes, left opens
 * When) or hold it (the row lifts, then a menu on release or a drag on move). The row stays put
 * for a vertical move, which is the page scrolling. `dx` is where the row sits now; `lifted` is
 * true from the moment the hold fires until the finger leaves. Only touch and pen are handled.
 */
export class RowTouch {
  dx = $state(0);
  /** A finger is moving the row: no easing while it follows. */
  following = $state(false);
  lifted = $state(false);
  /** A finger is down on the row (the platform's own context menu must not open then). */
  touching = false;

  #from: { x: number; y: number } | null = null;
  #axis: 'x' | 'y' | null = null;
  #pointer = -1;
  #sides: SwipeSides = { right: false, left: false };
  #quietUntil = 0;
  readonly #press: LongPress;

  constructor(private readonly opts: RowTouchOptions) {
    this.#press = new LongPress({
      onpress: () => {
        if (!this.#last || this.#axis === 'x') return;
        this.lifted = true;
        lightTick();
        this.opts.onhold(this.#last);
      },
    });
  }

  #last: PointerEvent | null = null;

  /** Whether a click now is the tail of a gesture and should be ignored. */
  get swallowsClick(): boolean {
    return performance.now() < this.#quietUntil;
  }

  down(e: PointerEvent): void {
    if (e.pointerType === 'mouse') return;
    if ((e.target as Element).closest(OWN_TAPS)) return;
    this.touching = true;
    this.#from = { x: e.clientX, y: e.clientY };
    this.#axis = null;
    this.#pointer = e.pointerId;
    this.#last = e;
    this.#sides = this.opts.sides();
    this.#press.start(e.clientX, e.clientY);
  }

  move(e: PointerEvent): void {
    if (!this.#from || e.pointerId !== this.#pointer || this.lifted) return;
    const dx = e.clientX - this.#from.x;
    const dy = e.clientY - this.#from.y;
    this.#press.move(e.clientX, e.clientY);
    if (this.#axis === null) {
      this.#axis = lockAxis(dx, dy);
      if (this.#axis === 'x' && (this.#sides.right || this.#sides.left)) {
        this.#press.cancel();
        this.following = true;
        (e.currentTarget as Element | null)?.setPointerCapture?.(e.pointerId);
      } else if (this.#axis !== null) {
        // A scroll, or a sideways move with nothing to do: leave the row alone.
        this.#press.cancel();
        this.#axis = this.#axis === 'x' ? 'y' : this.#axis;
      }
    }
    if (this.#axis === 'x') this.dx = rowOffset(dx, this.#sides);
  }

  up(e: PointerEvent): void {
    if (e.pointerId !== this.#pointer) return;
    const outcome = this.following ? rowSwipeOutcome(this.dx, this.#sides) : null;
    const wasLifted = this.lifted;
    const swiped = this.following;
    this.#reset();
    if (swiped || wasLifted) this.#quietUntil = performance.now() + CLICK_GUARD_MS;
    if (outcome === 'right') this.opts.onright();
    else if (outcome === 'left') this.opts.onleft();
  }

  cancel(): void {
    this.#reset();
  }

  #reset(): void {
    this.#press.cancel();
    this.#from = null;
    this.#axis = null;
    this.#pointer = -1;
    this.touching = false;
    this.following = false;
    this.lifted = false;
    this.dx = 0;
  }
}
