/** The Task whose row is a capture box right now (Quick edit): at most one. */
class QuickEditing {
  id = $state<string | null>(null);

  start(id: string): void {
    this.id = id;
  }

  stop(): void {
    this.id = null;
  }
}

export const quickEditing = new QuickEditing();

/** Whether `e` is the bare E key that starts a Quick edit. */
export const quickEditKey = (e: KeyboardEvent): boolean =>
  (e.key === 'e' || e.key === 'E') &&
  !e.metaKey &&
  !e.ctrlKey &&
  !e.altKey &&
  !e.shiftKey &&
  !e.repeat;
