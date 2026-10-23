import { urlParam } from '../../lib/router/url-param';

const PARAM = 'device';
/** Closes asked for within this long of each other are one close. */
const CLOSE_DEBOUNCE_MS = 400;

/**
 * Which paired device is open in its sheet. It lives in the URL (`/sync?device=‹id›`), so a
 * reload or a link opens it again and Back closes it: opening pushes a history entry, closing
 * steps back over it (or drops the parameter when the page was opened on a link).
 */
class DeviceSheetState {
  /** The open device's id, or none. */
  readonly id = $derived(urlParam.get(PARAM));
  /** Whether opening pushed the entry that closing should step back over. */
  #pushed = false;
  #closedAt = 0;

  /** Opens a device's sheet. */
  open(id: string): void {
    if (this.id === id) return;
    if (this.id === null) this.#pushed = true;
    urlParam.set(PARAM, id, this.id !== null);
  }

  /** Closes the sheet: back over the entry opening pushed, else drop the parameter. */
  close(): void {
    if (this.id === null || Date.now() - this.#closedAt < CLOSE_DEBOUNCE_MS) return;
    this.#closedAt = Date.now();
    if (this.#pushed) {
      this.#pushed = false;
      history.back();
    } else {
      urlParam.delete(PARAM, true);
    }
  }

  /** Drops the parameter without stepping back (the device is gone: forgotten, or never there). */
  dismiss(): void {
    this.#pushed = false;
    this.#closedAt = Date.now();
    if (this.id !== null) urlParam.delete(PARAM, true);
  }
}

export const deviceSheet = new DeviceSheetState();
