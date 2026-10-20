import { LeaveGuard } from '../../lib/leave-guard.svelte';

/** Asks "Save / Discard / Cancel" when leaving Settings with unsaved edits. */
export const leaveGuard = new LeaveGuard();
