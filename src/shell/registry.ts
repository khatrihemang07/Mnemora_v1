import { homeManifest } from '../modules/home/manifest';
import { journalManifest } from '../modules/journal/manifest';
import { settingsManifest } from '../modules/settings/manifest';

/** Every Module is registered here, and only here. */
export const modules = [homeManifest, journalManifest, settingsManifest];
