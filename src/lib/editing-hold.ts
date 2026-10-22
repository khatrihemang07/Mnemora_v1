import { commands } from './ipc/bindings';

/** How often an open editor renews its hold: Rust lets a hold lapse after two minutes. */
export const HOLD_RENEW_MS = 30_000;

export interface EditingHoldDeps {
  /** Takes or renews the hold on `scope`. */
  hold: (scope: string) => Promise<unknown>;
  /** Ends the hold on `scope`; Rust merges what waited for it. */
  release: (scope: string) => Promise<unknown>;
  /** Calls `fn` every `ms` until the returned function is called. */
  every: (fn: () => void, ms: number) => () => void;
}

/**
 * The holds one editor has on one record. While a person types in an Entry, or in one field of a
 * Task, changes other devices make to what they are editing wait (Rust keeps them apart and
 * merges them when the hold ends), so the text never jumps under the cursor. The hold is a lease:
 * it is renewed while the editor stays open and lapses by itself if the editor vanishes (the app
 * went to the background, the webview died), so it can never block sync for good. A failed call
 * is ignored: the worst case is that a change lands without waiting.
 */
export class EditingHold {
  readonly #renewals = new Map<string, () => void>();

  constructor(private readonly deps: EditingHoldDeps) {}

  /** Holds `scope` and keeps it held. Calling it again for a held scope does nothing. */
  take(scope: string): void {
    if (this.#renewals.has(scope)) return;
    const hold = () => void this.deps.hold(scope).catch(() => {});
    hold();
    this.#renewals.set(scope, this.deps.every(hold, HOLD_RENEW_MS));
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
    hold: (scope) => commands.editingHold(entity, entityId, scope),
    release: (scope) => commands.editingRelease(entity, entityId, scope),
    every: (fn, ms) => {
      const t = setInterval(fn, ms);
      return () => clearInterval(t);
    },
  });
}

/** The scope that holds a whole record (an Entry in its editor). */
export const WHOLE = '*';
