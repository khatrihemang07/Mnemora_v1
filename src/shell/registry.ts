import { aiManifest } from '../modules/ai/manifest';
import { homeManifest } from '../modules/home/manifest';
import { journalManifest } from '../modules/journal/manifest';
import { settingsManifest } from '../modules/settings/manifest';
import { syncManifest } from '../modules/sync/manifest';
import { todoManifest } from '../modules/todo/manifest';

/** Every Module is registered here, and only here. */
export const modules = [
  homeManifest,
  journalManifest,
  todoManifest,
  aiManifest,
  syncManifest,
  settingsManifest,
];
