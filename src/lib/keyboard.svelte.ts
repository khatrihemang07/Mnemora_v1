import { isKeyboardOpen, keyboardInset } from './keyboard-inset';
import { isMobileOs } from './platform';

/**
 * The on-screen keyboard, from `visualViewport`: the one source of its height. The app shell pads
 * its bottom by `inset` so everything above the keyboard stays visible. Always closed off mobile.
 */
class Keyboard {
  inset = $state(0);
  open = $derived(isKeyboardOpen(this.inset));

  constructor() {
    const vv = typeof window === 'undefined' ? null : window.visualViewport;
    if (!vv || !isMobileOs) return;
    const update = () => {
      this.inset = keyboardInset(window.innerHeight, vv.height, vv.offsetTop);
    };
    update();
    vv.addEventListener('resize', update);
    vv.addEventListener('scroll', update);
  }
}

export const keyboard = new Keyboard();
