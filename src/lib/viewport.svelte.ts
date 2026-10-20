import { isMobileOs } from './platform';

const QUERY = '(max-width: 719.98px)';

/** Phone layout: narrow window, or a mobile OS (touch sizes everywhere). */
class Viewport {
  narrow = $state(false);
  isPhone = $derived(this.narrow || isMobileOs);

  constructor() {
    if (typeof window === 'undefined') return;
    const mq = window.matchMedia(QUERY);
    this.narrow = mq.matches;
    mq.addEventListener('change', (e) => {
      this.narrow = e.matches;
    });
  }
}

export const viewport = new Viewport();
