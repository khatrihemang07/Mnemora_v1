import type { Attention } from '../../lib/ipc/bindings';

/**
 * The latest attention, kept where ⌘K can read it reactively: the palette asks for its hint while
 * it renders, so the "2 waiting" note follows the conflicts. SyncOverlay keeps it current.
 */
export const syncAttention = $state<{ current: Attention | undefined }>({ current: undefined });
