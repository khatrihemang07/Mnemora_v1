import type { GroupKey, Task, TodoList } from '../../lib/ipc/bindings';
import { adoptSummary } from './arrange';
import {
  autoScrollSpeed,
  insertIndex,
  isSamePlace,
  type Neighbours,
  neighboursAt,
  type RowBox,
  stepNeighbours,
} from './drag-math';
import { placeList } from './list-actions';
import { dropTaskOnSidebar, placeTask } from './task-actions';

/** How far the pointer travels before a press becomes a drag. */
const DRAG_SLOP_PX = 5;
/** A finger jitters more than a mouse: it needs a longer move before the held row lifts. */
const TOUCH_DRAG_SLOP_PX = 10;
/** Controls inside a row that keep their own clicks: a press on them never starts a drag. */
const NO_DRAG = 'button, a, input, textarea, select, [contenteditable="true"], [data-no-drag]';

/** What is being dragged. */
type Item = { kind: 'task'; task: Task } | { kind: 'list'; list: TodoList };

/** Where it would land if released now. */
type Drop =
  | { kind: 'place'; groupEl: HTMLElement; place: Neighbours; adopt: GroupKey[]; same: boolean }
  | { kind: 'sidebar'; target: string }
  | { kind: 'list-place'; place: Neighbours; same: boolean };

/** The line that shows where a drop would land: viewport coordinates. */
export interface DropLine {
  left: number;
  top: number;
  width: number;
}

/**
 * Dragging Tasks (and Lists in the sidebar) by pointer. A press on a row becomes a drag after a few
 * pixels; a floating label follows the pointer, a line shows the place, and a sidebar List or the
 * Inbox or Today lights up when the pointer is over it. Releasing drops: between two neighbours of
 * the manual order, into the group under the pointer (which adopts its day, Priority or List), or
 * on the sidebar. Esc cancels. Nothing here knows how a view is laid out beyond the `data-*`
 * markers its markup carries:
 *
 * - `data-task-group` + `data-adopt` (JSON `GroupKey[]`) on a group's container; `data-drag="off"`
 *   while the group is sorted; `data-task-row="‹id›"` on its rows; `data-group-end` where its
 *   "+ New task" row sits.
 * - `data-drop-nav` (`inbox`, `today` or `list:‹id›`) on a sidebar row that takes Tasks.
 * - `data-list-tree` on the sidebar's Lists, `data-list-row="‹id›"` on each draggable List.
 */
class DragController {
  item = $state.raw<Item | null>(null);
  x = $state(0);
  y = $state(0);
  line = $state.raw<DropLine | null>(null);
  /** The sidebar target under the pointer: `inbox`, `today` or `list:‹id›`. */
  nav = $state<string | null>(null);
  /** What a drop here would change about the Task, in words. */
  summary = $state<string | null>(null);

  /** What the labels need: today and the Lists by id. Set by the page that shows the rows. */
  context: { today: string; lists: ReadonlyMap<string, TodoList> } = {
    today: '',
    lists: new Map(),
  };

  #origin: { x: number; y: number } | null = null;
  /** Set while a held finger (not a mouse) owns the press: what a lift without moving does. */
  #tap: (() => void) | null = null;
  #canDrag = true;
  #pending: Item | null = null;
  #drop: Drop | null = null;
  #scroller: HTMLElement | null = null;
  #frame = 0;

  get active(): boolean {
    return this.item !== null;
  }

  /** The id of what is being dragged, for the row to dim itself. */
  get draggedId(): string | null {
    return this.item?.kind === 'task' ? this.item.task.id : (this.item?.list.id ?? null);
  }

  /** A press on a Task's row: starts a drag once the pointer has moved. */
  pressTask(e: PointerEvent, task: Task): void {
    this.#press(e, { kind: 'task', task });
  }

  /** A press on a List in the sidebar (its row is itself a button, so no control inside is checked). */
  pressList(e: PointerEvent, list: TodoList): void {
    this.#press(e, { kind: 'list', list }, false);
  }

  /**
   * A finger held on a Task's row long enough: it is picked up. Moving more than a few pixels
   * drags it (the page stops scrolling under the finger); lifting without moving calls `ontap`
   * (the row opens its menu), and a cancelled touch does nothing.
   */
  armTouch(e: PointerEvent, task: Task, ontap: () => void, canDrag: boolean): void {
    if (this.#pending || this.item) return;
    this.#tap = ontap;
    this.#canDrag = canDrag;
    this.#hold(e, { kind: 'task', task });
  }

  #press(e: PointerEvent, item: Item, guard = true): void {
    if (e.button !== 0 || e.pointerType === 'touch' || this.#pending || this.item) return;
    if (guard && e.target instanceof Element && e.target.closest(NO_DRAG)) return;
    this.#hold(e, item);
  }

  #hold(e: PointerEvent, item: Item): void {
    this.#pending = item;
    this.#origin = { x: e.clientX, y: e.clientY };
    this.x = e.clientX;
    this.y = e.clientY;
    this.#scroller = e.target instanceof Element ? scrollerOf(e.target) : null;
    window.addEventListener('pointermove', this.#move);
    window.addEventListener('pointerup', this.#up);
    window.addEventListener('pointercancel', this.#cancel);
    window.addEventListener('keydown', this.#key, true);
    // A held finger must not scroll the page: the first touchmove is still cancellable.
    window.addEventListener('touchmove', this.#hold_scroll, { passive: false });
  }

  #hold_scroll = (e: TouchEvent): void => {
    if (this.#tap) e.preventDefault();
  };

  #move = (e: PointerEvent): void => {
    this.x = e.clientX;
    this.y = e.clientY;
    if (!this.item) {
      const o = this.#origin;
      if (
        !o ||
        Math.hypot(e.clientX - o.x, e.clientY - o.y) <
          (this.#tap ? TOUCH_DRAG_SLOP_PX : DRAG_SLOP_PX)
      )
        return;
      // A row that cannot be arranged just lets go when the finger moves.
      if (this.#tap && !this.#canDrag) {
        this.#end();
        return;
      }
      this.#start();
    }
    this.#resolve();
  };

  #start(): void {
    this.item = this.#pending;
    document.body.dataset.dragging = '';
    window.getSelection()?.removeAllRanges();
    // The click that ends a drag must not open the Task under it.
    window.addEventListener('click', swallow, true);
    this.#loop();
  }

  /** Scrolls the page while the pointer is held near its top or bottom edge. */
  #loop = (): void => {
    this.#frame = requestAnimationFrame(() => {
      const box = this.#scroller?.getBoundingClientRect();
      if (this.#scroller && box) {
        const speed = autoScrollSpeed(this.y, box.top, box.bottom);
        if (speed !== 0) {
          this.#scroller.scrollTop += speed;
          this.#resolve();
        }
      }
      if (this.item) this.#loop();
    });
  };

  #resolve(): void {
    if (!this.item) return;
    const hits = document.elementsFromPoint(this.x, this.y);
    this.nav = null;
    this.line = null;
    this.summary = null;
    this.#drop = null;
    if (this.item.kind === 'list') {
      this.#resolveList(this.item.list);
      return;
    }
    const nav = hits.find(
      (el): el is HTMLElement => el instanceof HTMLElement && !!el.dataset.dropNav,
    );
    if (nav?.dataset.dropNav) {
      this.nav = nav.dataset.dropNav;
      this.#drop = { kind: 'sidebar', target: nav.dataset.dropNav };
      this.summary = this.#sidebarSummary(nav.dataset.dropNav);
      return;
    }
    this.#resolveTask(this.item.task);
  }

  #resolveTask(task: Task): void {
    const groups = [...document.querySelectorAll<HTMLElement>('[data-task-group]')].filter(
      (g) => g.dataset.drag !== 'off',
    );
    let best: { el: HTMLElement; gap: number } | null = null;
    for (const el of groups) {
      const r = el.getBoundingClientRect();
      if (this.x < r.left - 24 || this.x > r.right + 24) continue;
      const gap = this.y < r.top ? r.top - this.y : this.y > r.bottom ? this.y - r.bottom : 0;
      if (gap <= 28 && (!best || gap < best.gap)) best = { el, gap };
    }
    if (!best) return;
    const groupEl = best.el;
    const all = rowBoxes(groupEl);
    const rows = all.filter((r) => r.id !== task.id);
    const index = insertIndex(rows, this.y);
    const place = neighboursAt(
      rows.map((r) => r.id),
      index,
    );
    const adopt = JSON.parse(groupEl.dataset.adopt ?? '[]') as GroupKey[];
    const sameGroup = all.some((r) => r.id === task.id);
    const same =
      sameGroup &&
      isSamePlace(
        all.map((r) => r.id),
        task.id,
        place,
      ) &&
      adopt.every((k) => adoptSummary([k], task, this.context) === null);
    this.#drop = { kind: 'place', groupEl, place, adopt, same };
    this.line = lineAt(groupEl, rows, index);
    this.summary = adoptSummary(adopt, task, this.context);
  }

  #resolveList(list: TodoList): void {
    const tree = document.querySelector<HTMLElement>('[data-list-tree]');
    if (!tree) return;
    const t = tree.getBoundingClientRect();
    if (this.x < t.left || this.x > t.right || this.y < t.top - 16 || this.y > t.bottom + 16)
      return;
    const all = [...tree.querySelectorAll<HTMLElement>('[data-list-row]')].map(boxOf);
    const rows = all.filter((r) => r.id !== list.id);
    const index = insertIndex(rows, this.y);
    const place = neighboursAt(
      rows.map((r) => r.id),
      index,
    );
    const same = isSamePlace(
      all.map((r) => r.id),
      list.id,
      place,
    );
    this.#drop = { kind: 'list-place', place, same };
    const rowRects = [...tree.querySelectorAll<HTMLElement>('[data-list-row]')]
      .filter((el) => el.dataset.listRow !== list.id)
      .map((el) => el.getBoundingClientRect());
    const rect = rowRects[index] ?? rowRects.at(-1);
    if (!rect) return;
    this.line = {
      left: rect.left + 6,
      top: index < rowRects.length ? rect.top : rect.bottom,
      width: rect.width - 12,
    };
  }

  #sidebarSummary(target: string): string | null {
    if (this.item?.kind !== 'task') return null;
    if (target === 'today') return 'Today';
    if (target === 'inbox') return '📥 Inbox';
    const list = this.context.lists.get(target.slice('list:'.length));
    return list ? `${list.emoji ?? '≡'} ${list.name}` : null;
  }

  #up = (): void => {
    const item = this.item;
    const drop = this.#drop;
    const tap = this.item ? null : this.#tap;
    this.#end();
    if (tap) {
      tap();
      return;
    }
    if (!item || !drop) return;
    if (item.kind === 'list' && drop.kind === 'list-place' && !drop.same) {
      void placeList(item.list.id, drop.place.before, drop.place.after);
    } else if (item.kind === 'task' && drop.kind === 'place' && !drop.same) {
      void placeTask(item.task, { ...drop.place, adopt: drop.adopt });
    } else if (item.kind === 'task' && drop.kind === 'sidebar') {
      const target = drop.target;
      if (target === 'inbox') void dropTaskOnSidebar(item.task, { kind: 'inbox' });
      else if (target === 'today') void dropTaskOnSidebar(item.task, { kind: 'today' });
      else {
        const list = this.context.lists.get(target.slice('list:'.length));
        if (list) void dropTaskOnSidebar(item.task, { kind: 'list', list });
      }
    }
  };

  #cancel = (): void => this.#end();

  #key = (e: KeyboardEvent): void => {
    if (e.key !== 'Escape') return;
    e.preventDefault();
    e.stopPropagation();
    this.#end();
  };

  #end(): void {
    cancelAnimationFrame(this.#frame);
    window.removeEventListener('pointermove', this.#move);
    window.removeEventListener('pointerup', this.#up);
    window.removeEventListener('pointercancel', this.#cancel);
    window.removeEventListener('keydown', this.#key, true);
    window.removeEventListener('touchmove', this.#hold_scroll);
    this.#tap = null;
    // The click that follows the release is swallowed (it would open the Task); a press that never
    // became a drag lets it through. The click comes before the next task, so drop the guard then.
    if (this.item) setTimeout(() => window.removeEventListener('click', swallow, true), 0);
    delete document.body.dataset.dragging;
    this.item = null;
    this.line = null;
    this.nav = null;
    this.summary = null;
    this.#drop = null;
    this.#pending = null;
    this.#origin = null;
  }

  /**
   * ⌥↑ / ⌥↓: moves a Task one place within its group. Returns whether it moved. Groups that are
   * sorted (`data-drag="off"`) keep their order.
   */
  keyboardMove(task: Task, dir: -1 | 1): boolean {
    const row = document.querySelector<HTMLElement>(`[data-task-row="${CSS.escape(task.id)}"]`);
    const group = row?.closest<HTMLElement>('[data-task-group]');
    if (!group || group.dataset.drag === 'off') return false;
    const ids = rowBoxes(group).map((r) => r.id);
    const to = stepNeighbours(ids, task.id, dir);
    if (!to) return false;
    const adopt: GroupKey[] = [];
    void placeTask(task, { ...to, adopt }).then(() => {
      // The row is re-rendered in its new place: give it focus back.
      requestAnimationFrame(() => {
        document
          .querySelector<HTMLElement>(`[data-task-row="${CSS.escape(task.id)}"] [role="button"]`)
          ?.focus();
      });
    });
    return true;
  }
}

const swallow = (e: Event): void => {
  e.stopPropagation();
  e.preventDefault();
};

function boxOf(el: HTMLElement): RowBox {
  const r = el.getBoundingClientRect();
  return { id: el.dataset.taskRow ?? el.dataset.listRow ?? '', top: r.top, bottom: r.bottom };
}

/** A group's rows, top to bottom. */
function rowBoxes(groupEl: HTMLElement): RowBox[] {
  return [...groupEl.querySelectorAll<HTMLElement>('[data-task-row]')].map(boxOf);
}

/** The line for a drop at `index` among `rows` in a group: above that row, or after the last. */
function lineAt(groupEl: HTMLElement, rows: RowBox[], index: number): DropLine {
  const rowEls = [...groupEl.querySelectorAll<HTMLElement>('[data-task-row]')];
  const g = groupEl.getBoundingClientRect();
  const first = rowEls[0]?.getBoundingClientRect() ?? g;
  const visible = rows.map((r) => r.id);
  const el = rowEls.filter((r) => visible.includes(r.dataset.taskRow ?? ''));
  const at = el[index]?.getBoundingClientRect();
  const last = el.at(-1)?.getBoundingClientRect();
  const end = groupEl.querySelector('[data-group-end]')?.getBoundingClientRect();
  const top = at ? at.top : last ? last.bottom : (end?.top ?? g.bottom);
  return { left: first.left + 6, top, width: first.width - 12 };
}

/** The nearest ancestor that scrolls vertically. */
function scrollerOf(el: Element): HTMLElement | null {
  for (let p = el.parentElement; p; p = p.parentElement) {
    const overflow = getComputedStyle(p).overflowY;
    if (overflow === 'auto' || overflow === 'scroll') return p;
  }
  return null;
}

export const drag = new DragController();
