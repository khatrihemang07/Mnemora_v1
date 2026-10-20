/** Asks "Save / Discard / Cancel" when leaving Settings with unsaved edits. */
class LeaveGuard {
  open = $state(false);
  #resolve: ((leave: boolean) => void) | undefined;

  ask(): Promise<boolean> {
    this.open = true;
    return new Promise((resolve) => {
      this.#resolve = resolve;
    });
  }

  answer(leave: boolean) {
    this.open = false;
    this.#resolve?.(leave);
    this.#resolve = undefined;
  }
}

export const leaveGuard = new LeaveGuard();
