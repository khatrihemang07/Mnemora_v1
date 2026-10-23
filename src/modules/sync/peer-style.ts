import type { PeerState } from '../../lib/ipc/bindings';

/** The dot on a device's icon, by state (tokens only). */
export const DOT: Record<PeerState, string> = {
  up_to_date: 'bg-success',
  syncing: 'bg-hue-sync',
  not_nearby: 'bg-faint',
  paused: 'bg-warning',
  removed: 'bg-danger',
};

/** The status chip beside a device's name, by state. */
export const CHIP: Record<PeerState, string> = {
  up_to_date: 'bg-[color-mix(in_oklch,var(--success)_14%,transparent)] text-success',
  syncing: 'bg-hue-sync-soft text-hue-sync',
  not_nearby: 'bg-surface-2 text-muted',
  paused: 'bg-[color-mix(in_oklch,var(--warning)_16%,transparent)] text-ink',
  removed: 'bg-danger-soft text-danger',
};
