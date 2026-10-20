import type { SettingsChange } from '../ipc/bindings';

type Value = SettingsChange['value'];

const same = (a: unknown, b: unknown) => JSON.stringify(a) === JSON.stringify(b);

/** Unsaved Settings edits (N9-B). Only keys that differ from the saved value count as changes. */
export class SettingsDraft {
  #saved = $state<Record<string, Value>>({});
  #edits = $state<Record<string, Value>>({});

  constructor(saved: Record<string, Value> = {}) {
    this.#saved = saved;
  }

  get(key: string): Value {
    return key in this.#edits ? (this.#edits[key] as Value) : (this.#saved[key] as Value);
  }

  set(key: string, value: Value) {
    if (same(this.#saved[key], value)) delete this.#edits[key];
    else this.#edits[key] = value;
  }

  changes(): SettingsChange[] {
    return Object.entries(this.#edits).map(([key, value]) => ({ key, value }));
  }

  get dirty() {
    return Object.keys(this.#edits).length > 0;
  }

  discard() {
    this.#edits = {};
  }

  /** Saved values with edits applied; what the UI should preview. */
  merged(): Record<string, Value> {
    return { ...this.#saved, ...this.#edits };
  }

  /** Edits were saved successfully: they are the new saved values. */
  commit() {
    this.#saved = this.merged();
    this.#edits = {};
  }

  /** New saved snapshot arrived; keep edits that still differ from it. */
  rebase(saved: Record<string, Value>) {
    this.#saved = saved;
    for (const [key, value] of Object.entries(this.#edits)) {
      if (same(saved[key], value)) delete this.#edits[key];
    }
  }
}
