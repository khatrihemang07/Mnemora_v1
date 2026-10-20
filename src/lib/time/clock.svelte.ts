/**
 * One shared clock for the whole app: a single timer aligned to the second boundary,
 * running only while someone is subscribed and the document is visible.
 */
class Clock {
  now = $state(new Date());
  #subscribers = 0;
  #timer: ReturnType<typeof setTimeout> | undefined;

  subscribe(): () => void {
    this.#subscribers++;
    if (this.#subscribers === 1) {
      document.addEventListener('visibilitychange', this.#onVisibility);
      this.#start();
    }
    return () => {
      this.#subscribers--;
      if (this.#subscribers === 0) {
        document.removeEventListener('visibilitychange', this.#onVisibility);
        this.#stop();
      }
    };
  }

  #onVisibility = () => {
    if (document.hidden) this.#stop();
    else this.#start();
  };

  #start() {
    this.#stop();
    this.now = new Date();
    const tick = () => {
      this.now = new Date();
      this.#timer = setTimeout(tick, 1000 - (Date.now() % 1000));
    };
    this.#timer = setTimeout(tick, 1000 - (Date.now() % 1000));
  }

  #stop() {
    clearTimeout(this.#timer);
    this.#timer = undefined;
  }
}

export const clock = new Clock();
