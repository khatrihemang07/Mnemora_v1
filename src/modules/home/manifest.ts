import House from '~icons/lucide/house';
import type { ModuleManifest } from '../../lib/router';

export const homeManifest: ModuleManifest = {
  id: 'home',
  name: 'Home',
  icon: House,
  accentVar: '--hue-home',
  routes: [{ path: '/', component: () => import('./HomePage.svelte') }],
  nav: { order: 10 },
  required: true,
  bareKeys: [{ key: 'h', run: (ctx) => ctx.navigate('/') }],
};
