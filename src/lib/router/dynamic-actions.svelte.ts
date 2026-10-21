import { untrack } from 'svelte';
import type { ModuleAction } from './manifest';

/**
 * ⌘K actions that depend on data a Module loads (Todo's "Go to ‹List›" for every List). A Module's
 * always-on overlay sets them while it is mounted and clears them when it goes (it is not
 * mounted while the Module is switched off), so the palette lists them next to the manifest's own.
 */
class DynamicActions {
  #byModule = $state.raw<Record<string, ModuleAction[]>>({});

  of(moduleId: string): ModuleAction[] {
    return this.#byModule[moduleId] ?? [];
  }

  set(moduleId: string, actions: ModuleAction[]): void {
    // Read without tracking: a Module sets its actions from an effect, which must not depend on them.
    const rest = untrack(() => this.#byModule);
    this.#byModule = { ...rest, [moduleId]: actions };
  }

  clear(moduleId: string): void {
    const { [moduleId]: _gone, ...rest } = untrack(() => this.#byModule);
    this.#byModule = rest;
  }
}

export const dynamicActions = new DynamicActions();
