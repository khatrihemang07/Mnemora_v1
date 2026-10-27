import MessageSquare from '~icons/lucide/message-square';
import Sparkles from '~icons/lucide/sparkles';
import SquarePen from '~icons/lucide/square-pen';
import type { ModuleManifest } from '../../lib/router';
import { aiRequests } from './ai-requests.svelte';

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
  // A opens AI from anywhere, except on Todo, where G then A is Todo's chord for All. C and / act
  // only on the AI page: C copies a focused Journal Entry, and / searches the page it is on.
  bareKeys: [
    { key: 'a', when: (ctx) => !ctx.isActive('/todo'), run: (ctx) => ctx.navigate('/ai') },
    { key: 'c', when: (ctx) => ctx.isActive('/ai'), run: () => aiRequests.request('new') },
    { key: '/', when: (ctx) => ctx.isActive('/ai'), run: () => aiRequests.request('composer') },
  ],
  paletteActions: [
    {
      id: 'ask',
      label: 'Ask AI',
      icon: Sparkles,
      shortcut: 'A',
      run: (ctx) => {
        aiRequests.request('composer');
        ctx.navigate('/ai');
      },
    },
    {
      id: 'new-conversation',
      label: 'New Conversation',
      icon: SquarePen,
      shortcut: 'C',
      run: (ctx) => {
        aiRequests.request('new');
        ctx.navigate('/ai');
      },
    },
    {
      id: 'open-conversation',
      label: 'Open Conversation…',
      icon: MessageSquare,
      run: (ctx) => {
        aiRequests.request('history');
        ctx.navigate('/ai');
      },
    },
  ],
  settingsSection: {
    label: 'AI',
    icon: Sparkles,
    component: () => import('./ProviderSettings.svelte'),
  },
};
