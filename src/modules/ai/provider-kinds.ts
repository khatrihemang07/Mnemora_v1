import type {
  KeyState,
  NewProvider,
  ProviderKind,
  ProviderUpdate,
  ProviderView,
} from '../../lib/ipc/bindings';

/** How a kind takes a key: not at all, optionally, or it cannot answer without one. */
export type KeyRule = 'none' | 'optional' | 'required';

/** What the screen needs to know about one kind of Provider. Fixed behaviour; the Rust side agrees. */
export interface KindInfo {
  readonly id: ProviderKind;
  readonly name: string;
  /** One line on the kind step. */
  readonly description: string;
  /** The address a new Provider starts with, or the fixed one for a fixed kind. */
  readonly address: string;
  /** Fixed by the kind: shown read only and sent as it is. */
  readonly fixedAddress: boolean;
  readonly key: KeyRule;
  /** Whether the kind has paid Models behind the Provider's own switch. */
  readonly paid: boolean;
}

const WRAPPER: KindInfo = {
  id: 'wrapper',
  name: 'Local AI Wrapper',
  description: 'Your localAIWrapper on this machine or your network. No key.',
  address: 'http://127.0.0.1:8080/v1',
  fixedAddress: false,
  key: 'none',
  paid: false,
};

const OPENAI_COMPATIBLE: KindInfo = {
  id: 'openai_compatible',
  name: 'OpenAI-compatible',
  description:
    'Any endpoint that speaks the OpenAI chat API, such as OpenAI, Groq, Ollama or LM Studio.',
  address: '',
  fixedAddress: false,
  key: 'optional',
  paid: false,
};

const ANTHROPIC: KindInfo = {
  id: 'anthropic',
  name: 'Anthropic',
  description: 'Claude through your own API key.',
  address: 'https://api.anthropic.com/v1',
  fixedAddress: true,
  key: 'required',
  paid: false,
};

const OPENROUTER: KindInfo = {
  id: 'openrouter',
  name: 'OpenRouter',
  description: 'Hundreds of Models. Free Models by default; paid ones need your OK.',
  address: 'https://openrouter.ai/api/v1',
  fixedAddress: true,
  key: 'required',
  paid: true,
};

/** The kinds a Provider can be added as, in the order the kind step shows them. */
export const KINDS: readonly KindInfo[] = [WRAPPER, OPENAI_COMPATIBLE, ANTHROPIC, OPENROUTER];

/** The kind with this id. Every id the backend sends is one of the four, so the fallback is never used. */
export function kindOf(id: ProviderKind): KindInfo {
  return KINDS.find((kind) => kind.id === id) ?? WRAPPER;
}

/** The address a request carries: the kind's fixed one, or what the person typed, trimmed. */
export function addressFor(kind: KindInfo, typed: string): string {
  return kind.fixedAddress ? kind.address : typed.trim();
}

/** The request that adds a Provider. The kind is sent as chosen; paid Models are not part of it. */
export function createRequest(kind: KindInfo, name: string, typedAddress: string): NewProvider {
  return { kind: kind.id, name: name.trim(), address: addressFor(kind, typedAddress) };
}

/** The request that changes a Provider. `allow_paid` is false for a kind without paid Models. */
export function updateRequest(
  kind: KindInfo,
  name: string,
  typedAddress: string,
  allowPaid: boolean,
): ProviderUpdate {
  return {
    name: name.trim(),
    address: addressFor(kind, typedAddress),
    allow_paid: kind.paid && allowPaid,
  };
}

/** The status chip on a Provider row. `null` when the row shows none (a key is not needed). */
export function keyChip(state: KeyState): { label: string; tone: 'ok' | 'warn' } | null {
  if (state === 'missing') return { label: 'Needs a key', tone: 'warn' };
  if (state === 'saved') return { label: 'Key saved', tone: 'ok' };
  return null;
}

/** The muted line under an OpenRouter Provider's address while its paid switch is off. */
export function freeOnlyNote(provider: Pick<ProviderView, 'kind' | 'allow_paid'>): string | null {
  return provider.kind === 'openrouter' && !provider.allow_paid ? 'Free Models only' : null;
}

/** The paid switch in the form: its label and the line that says what it does. */
export const PAID_SWITCH = {
  label: 'Allow paid Models',
  hint: 'Free Models are always available. Paid Models are billed by OpenRouter to your account, so they can be chosen only while this is on.',
} as const;

/** The line under the key field, by how the kind takes a key. */
export function keyHint(rule: KeyRule): string {
  const lead = rule === 'required' ? 'Required for this kind.' : 'Optional for this kind.';
  return `${lead} Write-only: hidden as you type and never shown again. Stored on this device only.`;
}
