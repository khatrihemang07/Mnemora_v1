/**
 * Todo's desktop key map as one pure reducer: which key means what, given what is on screen. The
 * page feeds it every key press with a description of the scene (see `KeyScene`) and runs the
 * action it returns; nothing here touches the DOM, so every row of the map is a Vitest case.
 *
 * Bare keys follow ADR 0011: no modifier, never inside a field or editor, never while a menu or
 * Dialog is open. The G chord ("G then T") is the only state: when G was pressed.
 */

/** Where "G then a letter" can jump. */
export type GoTarget = 'today' | 'upcoming' | 'inbox' | 'all' | 'done';

const GO_KEYS: Record<string, GoTarget> = {
  t: 'today',
  u: 'upcoming',
  i: 'inbox',
  a: 'all',
  d: 'done',
};

/** A chord ends when the second key is slower than this. */
export const CHORD_MS = 1500;

export type KeyAction =
  /** ↑ ↓ moves the focus ring to the previous or next Task. */
  | { type: 'ring'; dir: 'up' | 'down' }
  /** Esc: drops the focus ring. */
  | { type: 'clear-ring' }
  /** Space: ticks an open Task, reopens a done one. */
  | { type: 'complete' }
  /** ⇧Space: Won't do, or Skip on a repeating Task. */
  | { type: 'wont' }
  | { type: 'open' }
  | { type: 'quick-edit' }
  | { type: 'when' }
  | { type: 'today' }
  | { type: 'move-list' }
  | { type: 'priority'; value: number }
  | { type: 'reorder'; dir: -1 | 1 }
  | { type: 'delete' }
  /** In the sheet: ⌘↑ ⌘↓ show the previous or next Task. */
  | { type: 'sheet-step'; dir: -1 | 1 }
  /** In the sheet: ⌘↵ closes it. */
  | { type: 'sheet-close' }
  | { type: 'go'; to: GoTarget }
  /** ⌘F: opens search, or puts the cursor back in its field. */
  | { type: 'search' };

/** The parts of a key event the map reads. */
export interface KeyEventLike {
  key: string;
  metaKey: boolean;
  ctrlKey: boolean;
  altKey: boolean;
  shiftKey: boolean;
  repeat?: boolean;
  isComposing?: boolean;
  defaultPrevented?: boolean;
}

/** What is on screen when the key goes down. */
export interface KeyScene {
  /** The key goes into a text field, an editor or the capture box. */
  typing: boolean;
  /** A menu, picker or Dialog is open (other than the Task sheet itself). */
  overlay: boolean;
  /** The Task sheet is open and the key belongs to it. */
  sheet: boolean;
  /** The Task a key would act on: the one with the focus ring (or focus), or none. */
  target: 'ring' | 'none';
  /** That Task is open (not done and not Won't do). */
  taskOpen: boolean;
  /** The key went to a button or link, which presses itself on Space and Enter. */
  onControl: boolean;
}

export interface KeyState {
  /** When G was pressed, while a chord waits for its second key. */
  goAt: number | null;
}

export const IDLE_KEYS: KeyState = { goAt: null };

export interface KeyResult {
  state: KeyState;
  action: KeyAction | null;
  /** The page should stop the browser's own handling of the key. */
  handled: boolean;
}

const none = (state: KeyState = IDLE_KEYS): KeyResult => ({ state, action: null, handled: false });
const act = (action: KeyAction, state: KeyState = IDLE_KEYS): KeyResult => ({
  state,
  action,
  handled: true,
});

/** Whether a modifier key (⌘ on macOS, Ctrl elsewhere) is held. */
const hasMod = (e: KeyEventLike) => e.metaKey || e.ctrlKey;

/** What `e` means in `scene`. `now` is the clock for the G chord. */
export function reduceKey(
  state: KeyState,
  e: KeyEventLike,
  scene: KeyScene,
  now: number,
): KeyResult {
  if (e.defaultPrevented || e.isComposing) return none();
  const key = e.key.length === 1 ? e.key.toLowerCase() : e.key;

  if (hasMod(e) && !e.altKey && !e.shiftKey) {
    if (scene.sheet) {
      if (e.key === 'ArrowUp' || e.key === 'ArrowDown') {
        return act({ type: 'sheet-step', dir: e.key === 'ArrowUp' ? -1 : 1 });
      }
      if (e.key === 'Enter') return act({ type: 'sheet-close' });
      return none();
    }
    // ⌘F works even from inside a field: it focuses the search field.
    if (key === 'f' && !scene.overlay) return act({ type: 'search' });
    return none();
  }
  if (hasMod(e) || scene.typing || scene.overlay) return none();

  // In the sheet only the keys that mean the same there: Priority and Won't do.
  if (scene.sheet) return sheetKey(e, key, scene);

  if (e.altKey) {
    if (e.shiftKey || e.repeat) return none();
    if (
      scene.target === 'ring' &&
      scene.taskOpen &&
      (e.key === 'ArrowUp' || e.key === 'ArrowDown')
    ) {
      return act({ type: 'reorder', dir: e.key === 'ArrowUp' ? -1 : 1 });
    }
    return none();
  }

  // G then a letter: the chord, if it is still fresh.
  const chord = state.goAt !== null && now - state.goAt <= CHORD_MS;
  if (chord && !e.shiftKey) {
    const to = GO_KEYS[key];
    if (to) return act({ type: 'go', to });
  }
  if (key === 'g' && !e.shiftKey && !e.repeat) return none({ goAt: now });

  if (e.key === 'ArrowUp' || e.key === 'ArrowDown') {
    return e.shiftKey ? none() : act({ type: 'ring', dir: e.key === 'ArrowUp' ? 'up' : 'down' });
  }

  if (e.key === ' ' && e.shiftKey) return taskKey(scene, { type: 'wont' }, true);
  if (e.shiftKey) return none();

  if (e.key === ' ') return scene.onControl ? none() : taskKey(scene, { type: 'complete' });
  if (e.key === 'Enter') return scene.onControl ? none() : taskKey(scene, { type: 'open' });
  if (e.key === 'Escape') return scene.target === 'ring' ? act({ type: 'clear-ring' }) : none();
  if (e.key === 'Backspace' || e.key === 'Delete') {
    return taskKey(scene, { type: 'delete' });
  }
  if (e.repeat) return none();
  if (/^[1-4]$/.test(e.key)) {
    return taskKey(scene, { type: 'priority', value: Number(e.key) }, true);
  }
  switch (key) {
    case 'e':
      return taskKey(scene, { type: 'quick-edit' }, true);
    case 'd':
      return taskKey(scene, { type: 'when' }, true);
    case 't':
      return taskKey(scene, { type: 'today' }, true);
    case 'm':
      return taskKey(scene, { type: 'move-list' });
    default:
      return none();
  }
}

/**
 * An action on the Task in `scene`: only the one with the focus ring (or focus) takes a key; a row
 * the pointer merely rests on does not (ADR 0013). `openOnly` actions mean nothing for a done Task.
 */
function taskKey(scene: KeyScene, action: KeyAction, openOnly = false) {
  if (scene.target !== 'ring') return none();
  if (openOnly && !scene.taskOpen) return none();
  return act(action);
}

function sheetKey(e: KeyEventLike, key: string, scene: KeyScene): KeyResult {
  if (e.altKey || e.repeat) return none();
  if (e.key === ' ' && e.shiftKey) return scene.taskOpen ? act({ type: 'wont' }) : none();
  if (e.shiftKey) return none();
  if (/^[1-4]$/.test(key) && scene.taskOpen) return act({ type: 'priority', value: Number(key) });
  return none();
}

/** Where the focus ring goes from `current` over `ids` (top to bottom): the ends hold. */
export function moveRing(ids: string[], current: string | null, dir: 'up' | 'down'): string | null {
  if (ids.length === 0) return null;
  const at = current === null ? -1 : ids.indexOf(current);
  if (at < 0) return (dir === 'down' ? ids[0] : ids[ids.length - 1]) ?? null;
  const next = ids[at + (dir === 'down' ? 1 : -1)];
  return next ?? current;
}

/** Where the ring lands when `id` goes away (done, deleted, moved): the next Task, else the previous. */
export function ringAfterRemoval(ids: string[], id: string): string | null {
  const at = ids.indexOf(id);
  if (at < 0) return null;
  return ids[at + 1] ?? ids[at - 1] ?? null;
}
