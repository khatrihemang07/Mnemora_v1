import { setTimer } from '../../lib/timing';

/** The progress bar fades in only if a sync is still running after this long (D9), so small ones never flash. */
export const PROGRESS_BAR_DELAY_MS = 600;

/**
 * Decides when a row's hairline progress bar is visible: not while a sync is young, yes once it
 * has run for `PROGRESS_BAR_DELAY_MS`, gone again as soon as it ends. Feed it the row's
 * "syncing" flag whenever that changes.
 */
export class BarGate {
  visible = false;
  #cancel: (() => void) | null = null;
  readonly #onChange: (visible: boolean) => void;
  readonly #timer: typeof setTimer;

  constructor(onChange: (visible: boolean) => void, timer: typeof setTimer = setTimer) {
    this.#onChange = onChange;
    this.#timer = timer;
  }

  update(syncing: boolean): void {
    if (syncing) {
      if (this.#cancel !== null || this.visible) return;
      this.#cancel = this.#timer(() => {
        this.#cancel = null;
        this.#show(true);
      }, PROGRESS_BAR_DELAY_MS);
      return;
    }
    this.stop();
  }

  /** Cancels a pending reveal and hides the bar. */
  stop(): void {
    this.#cancel?.();
    this.#cancel = null;
    this.#show(false);
  }

  #show(visible: boolean): void {
    if (this.visible === visible) return;
    this.visible = visible;
    this.#onChange(visible);
  }
}
