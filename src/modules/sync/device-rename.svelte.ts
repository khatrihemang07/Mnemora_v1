/**
 * The rename field of the device sheet. The name turns into a field; Enter or leaving it saves, Esc
 * keeps the old name. The sheet stays open after a save, showing the new name. The message under
 * the field always belongs to the latest try: a new try, or typing, clears the old one.
 */
export class DeviceRename {
  editing = $state(false);
  text = $state('');
  error = $state<string | null>(null);
  #saving = false;

  /** `save` throws an Error whose message is the reason (as `unwrap` does). */
  constructor(private readonly save: (name: string) => Promise<void>) {}

  start(current: string): void {
    this.text = current;
    this.error = null;
    this.editing = true;
  }

  /** The field was typed in: whatever was said about the last try no longer applies. */
  type(text: string): void {
    this.text = text;
    this.error = null;
  }

  cancel(): void {
    this.error = null;
    this.editing = false;
  }

  /** Saves unless the name is unchanged. Enter and blur may both ask: only one save runs. */
  async commit(current: string): Promise<void> {
    if (!this.editing || this.#saving) return;
    if (this.text.trim() === current) {
      this.cancel();
      return;
    }
    this.#saving = true;
    this.error = null;
    try {
      await this.save(this.text);
      this.editing = false;
    } catch (e) {
      this.error = e instanceof Error ? e.message : 'Could not rename this device';
    } finally {
      this.#saving = false;
    }
  }
}
