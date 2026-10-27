import type { ModelUse } from '../../lib/ipc/bindings';

/** The id typed in the picker, trimmed; `null` when nothing is typed. Any id is accepted. */
export function typedModelId(query: string): string | null {
  const id = query.trim();
  return id === '' ? null : id;
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
