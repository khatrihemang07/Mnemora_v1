import type { ModelUse } from '../../lib/ipc/bindings';

/** The id typed in the picker, trimmed; `null` when nothing is typed. Any id is accepted. */
export function typedModelId(query: string): string | null {
  const id = query.trim();
  return id === '' ? null : id;
}

/** The id Enter chooses in the search box: the typed id, as the "Use" row chooses it. `null` for any other key or nothing typed. */
export function choiceOnEnter(key: string, query: string): string | null {
  return key === 'Enter' ? typedModelId(query) : null;
}

/** The Models whose id contains the text (every Model when the text is empty), in the Provider's order. */
export function matchingModels(models: readonly string[], query: string): string[] {
  const needle = query.trim().toLowerCase();
  return needle === ''
    ? [...models]
    : models.filter((model) => model.toLowerCase().includes(needle));
}

/** The uses that went through one Provider. */
export function usesOf(uses: readonly ModelUse[], providerId: string): ModelUse[] {
  return uses.filter((use) => use.provider_id === providerId);
}

/** Whether a Model id is a free route. OpenRouter marks its free routes with `:free`, as the Rust rule does. */
export function isFreeModel(model: string): boolean {
  return model.endsWith(':free');
}

/** The muted reason a paid Model row is disabled. */
export const PAID_MODEL_REASON = 'Paid Model. Turn on paid Models for this Provider to choose it.';

/** How one Model row is shown: whether it is paid, whether it can be chosen, and why not. */
export interface ModelRowState {
  readonly paid: boolean;
  readonly disabled: boolean;
  readonly reason: string | null;
}

/**
 * A Model row in the picker. Only a kind with paid Models can have paid rows: a row is paid when the
 * Provider's list says so, or when its id is not a free route (recent and most-used rows are not
 * listed by the Provider, so the id decides for them). A paid row is disabled while the switch is off.
 */
export function modelRowState(
  model: string,
  options: { paidKind: boolean; paidIds: ReadonlySet<string>; allowPaid: boolean },
): ModelRowState {
  const paid = options.paidKind && (options.paidIds.has(model) || !isFreeModel(model));
  const disabled = paid && !options.allowPaid;
  return { paid, disabled, reason: disabled ? PAID_MODEL_REASON : null };
}
