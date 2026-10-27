/**
 * What a key or a palette action asks the AI page to do: focus the question box, open the history,
 * or start a new Conversation. Held until the page takes it, so an action that navigates here first
 * still lands once the page has mounted.
 */
export type AiRequest = 'composer' | 'history' | 'new';

class AiRequests {
  pending = $state<AiRequest | null>(null);

  request(kind: AiRequest): void {
    this.pending = kind;
  }

  /** Returns the waiting request and clears it. */
  take(): AiRequest | null {
    const kind = this.pending;
    this.pending = null;
    return kind;
  }
}

export const aiRequests = new AiRequests();
