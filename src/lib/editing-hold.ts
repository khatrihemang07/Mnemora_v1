import { commands } from './ipc/bindings';

/** How often an open editor renews its hold: Rust lets a hold lapse after two minutes. */
export const HOLD_RENEW_MS = 30_000;

export interface EditingHoldDeps {
  /** Takes or renews the hold on `scope`. Says whether Sync is on (off: nothing to renew). */
  hold: (scope: string) => Promise<boolean>;
  /** Ends the hold on `scope`; Rust merges what waited for it. */
  release: (scope: string) => Promise<unknown>;
  /** Calls `fn` every `ms` until the returned function is called. */
  repeat: (fn: () => void, ms: number) => () => void;
  /** Whether the window is showing; a hidden window renews nothing and its holds lapse. */
  visible: () => boolean;
  /** Calls `fn` whenever the window is shown again, until the returned function is called. */
  onShown: (fn: () => void) => () => void;
}

/**
 * The holds one editor has on one record. While a person types in an Entry, or in one field of a
 * Task, changes other devices make to what they are editing wait (Rust keeps them apart and
 * merges them when the hold ends), so the text never jumps under the cursor. The hold is a lease:
 * it is renewed while the editor stays open and the window shows, and lapses by itself if the
 * editor vanishes (the app went to the background, the webview died), so it can never block sync
 * for good. With Sync off no timer runs. A failed call is ignored: the worst case is that a
 * change lands without waiting.
 */
export class EditingHold {
  /** Scope to the function that stops its renewals (a no-op until Rust says Sync is on). */
  readonly #renewals = new Map<string, () => void>();

  #unwatch: (() => void) | null = null;

  constructor(private readonly deps: EditingHoldDeps) {}

  /** Holds `scope` and keeps it held. Calling it again for a held scope does nothing. */
  take(scope: string): void {
    if (this.#renewals.has(scope)) return;
    this.#unwatch ??= this.deps.onShown(() => this.renewNow());
    let stop = () => {};
    let released = false;
    this.#renewals.set(scope, () => {
      released = true;
      stop();
    });
    void this.#renew(scope).then((syncOn) => {
      if (!syncOn || released) return;
      stop = this.deps.repeat(() => void this.#renew(scope), HOLD_RENEW_MS);
    });
  }

  /** Renews every hold now (the window came back). */
  renewNow(): void {
    for (const scope of this.#renewals.keys()) void this.#renew(scope);
  }

  async #renew(scope: string): Promise<boolean> {
    if (!this.deps.visible()) return false;
    return this.deps.hold(scope).catch(() => false);
  }

  /** Whether `scope` is held. */
  has(scope: string): boolean {
    return this.#renewals.has(scope);
  }

  /** Lets go of `scope`, once the text is saved. */
  async release(scope: string): Promise<void> {
    const stop = this.#renewals.get(scope);
    if (!stop) return;
    stop();
    this.#renewals.delete(scope);
    if (this.#renewals.size === 0) {
      this.#unwatch?.();
      this.#unwatch = null;
    }
    await this.deps.release(scope).catch(() => {});
  }

  /** Lets go of everything this editor holds. */
  async releaseAll(): Promise<void> {
    await Promise.all([...this.#renewals.keys()].map((scope) => this.release(scope)));
  }
}

/** The holds of an editor on the record `entityId` of the syncable `entity`. */
export function editingHold(entity: string, entityId: string): EditingHold {
  return new EditingHold({
    hold: async (scope) => {
      const result = await commands.editingHold(entity, entityId, scope);
      return result.status === 'ok' && result.data === true;
    },
    release: (scope) => commands.editingRelease(entity, entityId, scope),
    repeat: (fn, ms) => {
      const t = setInterval(fn, ms);
      return () => clearInterval(t);
    },
    visible: () => document.visibilityState !== 'hidden',
    onShown: (fn) => {
      const shown = () => document.visibilityState === 'visible' && fn();
      document.addEventListener('visibilitychange', shown);
      return () => document.removeEventListener('visibilitychange', shown);
    },
  });
}

/** The scope that holds a whole record (an Entry in its editor). */
export const WHOLE = '*';
