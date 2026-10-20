/** Asks the user whether to leave a screen with unsaved work; `answer` settles the question. */
export class LeaveGuard {
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
