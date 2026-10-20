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
    // `data-phone` on <html> drives the `phone:` variant. Set here, synchronously at module load,
    // so the first paint already agrees with the layout switch.
    this.mark();
    mq.addEventListener('change', (e) => {
      this.narrow = e.matches;
      this.mark();
    });
  }

  private mark() {
    document.documentElement.toggleAttribute('data-phone', this.narrow || isMobileOs);
  }
}

export const viewport = new Viewport();
