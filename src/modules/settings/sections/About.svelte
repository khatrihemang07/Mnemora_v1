<!-- About: version, logs, diagnostics and the log level (a device setting, applies at once). -->
<script lang="ts">
  import { createQuery, useQueryClient } from '@tanstack/svelte-query';
  import { commands } from '../../../lib/ipc/bindings';
  import { recentUiErrors } from '../../../lib/ipc/errors';
  import { unwrap } from '../../../lib/ipc/unwrap';
  import { os } from '../../../lib/platform';
  import { appInfoQuery, deviceQuery } from '../../../lib/settings/queries';
  import Button from '../../../lib/ui/Button.svelte';
  import Select from '../../../lib/ui/Select.svelte';
  import SettingRow from '../../../lib/ui/SettingRow.svelte';
  import SettingSection from '../../../lib/ui/SettingSection.svelte';
  import { notify, notifyError } from '../../../lib/ui/toast';
  import { buildDiagnostics } from '../diagnostics';

  const client = useQueryClient();
  const info = createQuery(() => appInfoQuery);
  const device = createQuery(() => deviceQuery);
  const level = $derived(device.data?.log_level ?? 'info');
  const levels = ['error', 'warn', 'info', 'debug', 'trace'].map((l) => ({ value: l, label: l }));

  async function setLevel(next: string) {
    try {
      await unwrap(commands.deviceSetLogLevel(next));
      client.setQueryData(deviceQuery.queryKey, { ...device.data, log_level: next });
    } catch (e) {
      notifyError(e instanceof Error ? e.message : 'Could not change the log level');
    }
  }

  async function openLogs() {
    try {
      await unwrap(commands.openLogDir());
    } catch (e) {
      notifyError(e instanceof Error ? e.message : 'Could not open the log folder');
    }
  }

  async function copyDiagnostics() {
    if (!info.data) return;
    const text = buildDiagnostics({
      version: info.data.version,
      environment: info.data.environment,
      os,
      userAgent: navigator.userAgent,
      errors: recentUiErrors(),
    });
    try {
      await navigator.clipboard.writeText(text);
      notify('Diagnostics copied');
    } catch {
      notifyError('Could not copy diagnostics');
    }
  }
</script>

<SettingSection title="About">
  <SettingRow label="Version"><span class="font-mono">{info.data?.version ?? '…'}</span></SettingRow>
  <SettingRow label="Environment"><span class="capitalize">{info.data?.environment ?? '…'}</span></SettingRow>
  <SettingRow label="Log folder" hint="Where Mnemora writes its log files.">
    <Button size="sm" onclick={openLogs}>Open log folder</Button>
  </SettingRow>
  <SettingRow label="Diagnostics" hint="Version, Environment, OS and recent errors. No personal data.">
    <Button size="sm" disabled={!info.data} onclick={copyDiagnostics}>Copy diagnostics</Button>
  </SettingRow>
  <SettingRow label="Log level" hint="Applies immediately on this device.">
    <Select label="Log level" value={level} options={levels} onchange={setLevel} />
  </SettingRow>
</SettingSection>
