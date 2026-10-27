import Sparkles from '~icons/lucide/sparkles';
import type { ModuleManifest } from '../../lib/router';

/** The AI Module: off until the person turns it on (Settings ▸ Modules). Read-only. */
export const aiManifest: ModuleManifest = {
  id: 'ai',
  name: 'AI',
  description: 'Ask questions about your Journal and Tasks.',
  icon: Sparkles,
  accentVar: '--hue-ai',
  routes: [{ path: '/ai', component: () => import('./AiPage.svelte') }],
  nav: { order: 40 },
  defaultEnabled: false,
};
