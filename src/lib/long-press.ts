import { setTimer } from './timing';

/** How long a touch must rest before it counts as a long-press. */
export const LONG_PRESS_MS = 500;
/** How far a finger may drift before it is a scroll, not a press. */
export const LONG_PRESS_SLOP_PX = 10;

export interface LongPressOptions {
  onpress: (x: number, y: number) => void;
  /** Injected so tests can drive time; returns a cancel function. */
  setTimer?: (fn: () => void, ms: number) => () => void;
  ms?: number;
  slop?: number;
}

/**
 * Detects a long-press from pointer positions: `start`, then `move`s, then `end` (or `cancel`).
 * Fires `onpress` once if the pointer stays within the slop for the whole delay.
 */
export class LongPress {
  /** True from the moment it fires until the next `start`: the caller swallows the click that follows. */
  fired = false;
  private origin: { x: number; y: number } | null = null;
  private cancelTimer: (() => void) | null = null;
  private readonly opts: Required<LongPressOptions>;

  constructor(opts: LongPressOptions) {
    this.opts = {
      setTimer,
      ms: LONG_PRESS_MS,
      slop: LONG_PRESS_SLOP_PX,
      ...opts,
    };
  }

  start(x: number, y: number): void {
    this.cancel();
    this.fired = false;
    this.origin = { x, y };
    this.cancelTimer = this.opts.setTimer(() => {
      this.cancelTimer = null;
      this.fired = true;
      this.opts.onpress(x, y);
    }, this.opts.ms);
  }

  move(x: number, y: number): void {
    if (!this.origin) return;
    if (Math.hypot(x - this.origin.x, y - this.origin.y) > this.opts.slop) this.cancel();
  }

  /** The pointer lifted, was interrupted or scrolled away. */
  cancel(): void {
    this.cancelTimer?.();
    this.cancelTimer = null;
    this.origin = null;
  }
}
