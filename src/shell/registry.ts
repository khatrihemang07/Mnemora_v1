import { homeManifest } from '../modules/home/manifest';
import { settingsManifest } from '../modules/settings/manifest';

/** Every Module is registered here, and only here. */
export const modules = [homeManifest, settingsManifest];
