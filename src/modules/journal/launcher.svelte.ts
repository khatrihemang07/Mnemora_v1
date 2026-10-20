/** What the shell asked the Journal to do from outside it (⌘N, ⌘K, the Home card). */
export type JournalRequest = 'compose' | 'go-to-date';

/** A request made before the Journal is showing waits here; the Journal takes it once it is. */
class Launcher {
  pending = $state<JournalRequest | null>(null);
}
export const launcher = new Launcher();

/** Open the composer on Today, focused. */
export const requestCompose = () => {
  launcher.pending = 'compose';
};

/** Open the Journal's calendar. */
export const requestGoToDate = () => {
  launcher.pending = 'go-to-date';
};
