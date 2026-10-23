import type { DeviceDetail, ModuleVersion } from '../../lib/ipc/bindings';
import { relativeTime } from '../../lib/time/relative';

/** "Core" for `core`: the Module's name as the sheet shows it. */
function moduleName(id: string): string {
  return id.charAt(0).toUpperCase() + id.slice(1);
}

/** The order the sheet lists Modules in: the content Modules first, as in the sidebar. */
const ORDER = ['journal', 'todo', 'sync', 'core'];

/** "Mnemora 0.9.2 · Journal 3 · Todo 9 · Core 4"; just the Modules while the app version is unknown. */
export function versionsText(
  detail: Pick<DeviceDetail, 'app_version' | 'data_versions'>,
): string | null {
  const rank = (m: ModuleVersion) => {
    const at = ORDER.indexOf(m.module);
    return at < 0 ? ORDER.length : at;
  };
  const modules = [...detail.data_versions]
    .sort((a, b) => rank(a) - rank(b) || a.module.localeCompare(b.module))
    .map((m) => `${moduleName(m.module)} ${m.version}`);
  const parts = [detail.app_version === null ? null : `Mnemora ${detail.app_version}`, ...modules];
  const text = parts.filter((p): p is string => p !== null).join(' · ');
  return text === '' ? null : text;
}

/** One address with when it last worked: "192.168.1.31:47311", "worked 2 min ago". */
export interface AddressLine {
  address: string;
  when: string;
}

export function addressLines(
  detail: Pick<DeviceDetail, 'addresses'>,
  now: Date = new Date(),
): AddressLine[] {
  return detail.addresses.map((a) => ({
    address: a.address,
    when:
      a.last_ok_at === null
        ? 'not tried yet'
        : `worked ${relativeTime(new Date(a.last_ok_at), now)}`,
  }));
}

/** "14 Oct 2026, by Hemang's MacBook Pro". */
export function pairedText(detail: Pick<DeviceDetail, 'paired_at' | 'paired_by'>): string {
  const date = new Date(detail.paired_at).toLocaleDateString(undefined, { dateStyle: 'medium' });
  return detail.paired_by === null ? date : `${date}, by ${detail.paired_by}`;
}
