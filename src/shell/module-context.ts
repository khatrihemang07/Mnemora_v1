import type { ModuleContext } from '../lib/router';
import { router } from './router';

/** What Module actions (⌘N, palette) get from the shell. */
export const moduleContext: ModuleContext = {
  navigate: (path) => void router.navigate(path),
  isActive: (path) => router.isActive(path),
};
