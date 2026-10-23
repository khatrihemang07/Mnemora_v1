/**
 * A TypeScript copy of Sync's merge rules for the browser mock, so the simulated "Pixel 8
 * (simulated)" can sync with the mock's data and the Sync pages have something real to show.
 *
 * It is small on purpose: a device's data is its LOG of records (what Rust keeps in change_log),
 * and the state is a pure function of the union of two logs, so both devices always agree:
 *  - a Value or Order unit (a field, or an atomic group such as a Task's When) is taken whole from
 *    the newest record that touched it;
 *  - a Text unit is the replay of its records in clock order, each one merged three-way against
 *    the text it replaced (`prev`) when its author had not seen the text before it
 *    (crates/sync/src/texts.rs, textmerge.rs);
 *  - then the rules on top: Lists with one name merge into the older, a Task whose List is gone is
 *    placed in the Inbox, a Preference is shared unless it is look and feel.
 *
 * `fixtures/sync-merge-rules.json` guards this file: src-tauri/src/sync_merge_fixture_tests.rs
 * runs every case through two real devices and mock-sync-merge.test.ts through this copy. Change a
 * rule in Rust and a case fails here until the copy follows (ADR 0009).
 */

import { keyAfter } from './mock-todo-order';

export type Json = string | number | boolean | null | Json[];

/** Whether two values are the same (lists by their items). */
export const same = (a: Json | undefined, b: Json | undefined) =>
  a === b || JSON.stringify(a ?? null) === JSON.stringify(b ?? null);
export type Entity = 'task' | 'list' | 'comment' | 'entry' | 'setting';

type Class = 'value' | 'text' | 'order';
interface Def {
  cls: Class;
  group?: string;
}

const v = (group?: string): Def => ({ cls: 'value', group });
const t = (group?: string): Def => ({ cls: 'text', group });
const o: Def = { cls: 'order' };

/** Mirrors the entities' fields in crates/todo, crates/journal and crates/core (sync.rs files). */
const SCHEMA: Record<Entity, Record<string, Def>> = {
  task: {
    list_id: v(),
    title: t('title'),
    understood: v('title'),
    notes: t(),
    due_date: v('when'),
    due_time: v('when'),
    repeat_freq: v('when'),
    repeat_every: v('when'),
    repeat_days: v('when'),
    repeat_mode: v('when'),
    priority: v(),
    state: v('state'),
    closed_at: v('state'),
    repeat_of: v(),
    sort_key: o,
    created_at: v(),
    deleted_at: v(),
    skipped: v(),
  },
  list: {
    name: v(),
    emoji: v(),
    task_sort: v(),
    group_by: v(),
    sort_key: o,
    created_at: v(),
    deleted_at: v(),
  },
  comment: {
    body: t('body'),
    edited: v('body'),
    task_id: v(),
    created_at: v(),
    deleted_at: v(),
  },
  entry: {
    body: t(),
    page_date: v(),
    written_at: v('written'),
    written_offset_min: v('written'),
    written_tz: v('written'),
    created_at: v(),
    deleted_at: v(),
  },
  setting: { value: v() },
};

/** Look and feel stays on each device (crates/core device.rs LOOK_KEYS). */
export const LOOK_KEYS = [
  'core.theme',
  'core.density',
  'core.text_size',
  'core.reduce_motion',
  'core.phone_tabs',
  'core.phone_tabs_edited',
];

export interface Stamp {
  ms: number;
  counter: number;
  device: string;
}

/** One change of one record: what a device writes to its change log and sends. */
export interface Rec {
  entity: Entity;
  id: string;
  stamp: Stamp;
  seq: number;
  /** The author's values for what it changed (a touched group is complete). */
  fields: Record<string, Json>;
  /** What each of those fields held on the author's device before. */
  prev: Record<string, Json>;
  /**
   * For a delete: the highest `seq` the author held of each other device's records of this kind
   * of entity. An edit above it was made without knowing of the delete (Rust: change_log.seen).
   */
  seen?: Record<string, number>;
}

const cmp = (a: Rec, b: Rec) =>
  a.stamp.ms - b.stamp.ms ||
  a.stamp.counter - b.stamp.counter ||
  (a.stamp.device < b.stamp.device ? -1 : a.stamp.device > b.stamp.device ? 1 : 0) ||
  a.seq - b.seq;

export interface Doc {
  entity: Entity;
  id: string;
  values: Record<string, Json>;
}

export interface TextClash {
  entity: Entity;
  id: string;
  field: string;
  base: string;
  ours: string;
  oursDevice: string;
  oursAt: number;
  theirs: string;
  theirsDevice: string;
  theirsAt: number;
  pieces: Piece[];
}

/** Two changes of one unit made without seeing each other: the newer was kept. */
export interface ValueClash {
  entity: Entity;
  id: string;
  unit: string;
  kept: Record<string, Json>;
  keptDevice: string;
  replaced: Record<string, Json>;
  replacedDevice: string;
  at: number;
}

export interface Outcome {
  docs: Doc[];
  /** Text clashes that wait for a person (none on a deleted record). */
  conflicts: TextClash[];
  /** Text clashes on a record that is deleted: resolved at once and logged. */
  deletedConflicts: TextClash[];
  values: ValueClash[];
  /** A record deleted by one device and edited later by another. */
  deleteEdits: { entity: Entity; id: string; deleter: string; editor: string; at: number }[];
  /** A Task put in the Inbox because its List is gone. */
  inbox: { task: string; list: string; deleter: string; at: number }[];
  /** Lists that merged into an older one with the same name. */
  mergedLists: { from: string; into: string; fromDevice: string; at: number }[];
}

// ---------------------------------------------------------------------------------- text merge

export type Piece = { agreed: string } | { clash: { base: string; ours: string; theirs: string } };

interface Hunk {
  b0: number;
  b1: number;
  n0: number;
  n1: number;
}

/** Where `next` differs from `base`, as runs of changed positions (a longest-common-subsequence diff). */
function hunksOf(base: string[], next: string[]): Hunk[] {
  const w = next.length + 1;
  const lcs = new Array<number>((base.length + 1) * w).fill(0);
  const at = (i: number, j: number) => lcs[i * w + j] ?? 0;
  for (let i = base.length - 1; i >= 0; i--)
    for (let j = next.length - 1; j >= 0; j--)
      lcs[i * w + j] =
        base[i] === next[j] ? at(i + 1, j + 1) + 1 : Math.max(at(i + 1, j), at(i, j + 1));
  const out: Hunk[] = [];
  let open: Hunk | null = null;
  let i = 0;
  let j = 0;
  while (i < base.length || j < next.length) {
    if (i < base.length && j < next.length && base[i] === next[j]) {
      if (open) out.push(open);
      open = null;
      i++;
      j++;
      continue;
    }
    open ??= { b0: i, b1: i, n0: j, n1: j };
    if (j < next.length && (i === base.length || at(i, j + 1) >= at(i + 1, j))) j++;
    else i++;
    open.b1 = i;
    open.n1 = j;
  }
  if (open) out.push(open);
  return out;
}

const overlap = (a: Hunk, b: Hunk) => {
  if (a.b0 < b.b1 && b.b0 < a.b1) return true;
  const empty = (h: Hunk) => h.b0 === h.b1;
  const inside = (p: number, h: Hunk) => h.b0 < p && p < h.b1;
  return (
    (empty(a) && empty(b) && a.b0 === b.b0) ||
    (empty(a) && inside(a.b0, b)) ||
    (empty(b) && inside(b.b0, a))
  );
};

type Seg = { same: string[] } | { both: { base: string[]; ours: string[]; theirs: string[] } };

/** The side's version of base[from, to) with its hunks applied. */
function applied(base: string[], next: string[], from: number, to: number, hs: Hunk[]): string[] {
  const out: string[] = [];
  let at = from;
  for (const h of hs) {
    out.push(...base.slice(at, h.b0), ...next.slice(h.n0, h.n1));
    at = h.b1;
  }
  return [...out, ...base.slice(at, to)];
}

function mergeSeq(base: string[], ours: string[], theirs: string[]): Seg[] {
  const all = [
    ...hunksOf(base, ours).map((h) => ({ h, mine: true })),
    ...hunksOf(base, theirs).map((h) => ({ h, mine: false })),
  ].sort((x, y) => x.h.b0 - y.h.b0 || x.h.b1 - y.h.b1 || Number(!x.mine) - Number(!y.mine));
  interface Region {
    ours: Hunk[];
    theirs: Hunk[];
  }
  const regions: Region[] = [];
  let cur: Region = { ours: [], theirs: [] };
  for (const { h, mine } of all) {
    const others = mine ? cur.theirs : cur.ours;
    if ((cur.ours.length || cur.theirs.length) && !others.some((x) => overlap(h, x))) {
      regions.push(cur);
      cur = { ours: [], theirs: [] };
    }
    (mine ? cur.ours : cur.theirs).push(h);
  }
  if (cur.ours.length || cur.theirs.length) regions.push(cur);

  const out: Seg[] = [];
  let at = 0;
  for (const r of regions) {
    const hs = [...r.ours, ...r.theirs];
    const from = Math.min(...hs.map((h) => h.b0));
    const to = Math.max(...hs.map((h) => h.b1));
    if (from > at) out.push({ same: base.slice(at, from) });
    at = to;
    const mine = applied(base, ours, from, to, r.ours);
    const yours = applied(base, theirs, from, to, r.theirs);
    if (r.theirs.length === 0) out.push({ same: mine });
    else if (r.ours.length === 0 || mine.join('') === yours.join('')) out.push({ same: yours });
    else out.push({ both: { base: base.slice(from, to), ours: mine, theirs: yours } });
  }
  if (at < base.length) out.push({ same: base.slice(at) });
  return out;
}

const linesOf = (s: string) => s.match(/[^\n]*\n|[^\n]+/g) ?? [];

/** Words, runs of spaces and single line breaks (crates/sync/src/textmerge.rs `tokens_of`). */
const tokensOf = (s: string) => s.match(/\n|[^\S\n]+|[^\s]+/g) ?? [];

class Pieces {
  list: Piece[] = [];
  agreed(text: string) {
    if (!text) return;
    const last = this.list[this.list.length - 1];
    if (last && 'agreed' in last) last.agreed += text;
    else this.list.push({ agreed: text });
  }
}

const commonPrefix = (a: string[], b: string[]) => {
  let n = 0;
  while (n < a.length && n < b.length && a[n] === b[n]) n++;
  return n;
};

/** A clash of tokens: what the sides share at either end is agreed text, not part of the clash. */
function settle(base: string[], ours: string[], theirs: string[], out: Pieces) {
  const pre = commonPrefix(ours, theirs);
  const rev = (a: string[]) => a.slice(pre).reverse();
  const suf = commonPrefix(rev(ours), rev(theirs));
  const mid = (a: string[]) => a.slice(pre, a.length - suf);
  const [mo, mt] = [mid(ours), mid(theirs)];
  out.agreed(ours.slice(0, pre).join(''));
  if ((mo.length === 0 && ours.length > 0) || (mt.length === 0 && theirs.length > 0)) {
    out.agreed(mo.join(''));
    out.agreed(mt.join(''));
  } else {
    const bPre = commonPrefix(base, ours.slice(0, pre));
    const rest = base.slice(bPre);
    const bSuf = commonPrefix([...rest].reverse(), ours.slice(ours.length - suf).reverse());
    out.list.push({
      clash: {
        base: rest.slice(0, rest.length - bSuf).join(''),
        ours: mo.join(''),
        theirs: mt.join(''),
      },
    });
  }
  out.agreed(ours.slice(ours.length - suf).join(''));
}

/** Three-way text merge: `ours` and `theirs` are both edits of `base`; a clash shows `theirs` (the newer). */
export function textMerge(base: string, ours: string, theirs: string): Piece[] {
  const out = new Pieces();
  if (ours === theirs || base === theirs) out.agreed(ours);
  else if (base === ours) out.agreed(theirs);
  else
    for (const seg of mergeSeq(linesOf(base), linesOf(ours), linesOf(theirs))) {
      if ('same' in seg) out.agreed(seg.same.join(''));
      else {
        const [b, a, c] = [
          seg.both.base.join(''),
          seg.both.ours.join(''),
          seg.both.theirs.join(''),
        ];
        if (a === c) out.agreed(a);
        else
          for (const s of mergeSeq(tokensOf(b), tokensOf(a), tokensOf(c))) {
            if ('same' in s) out.agreed(s.same.join(''));
            else settle(s.both.base, s.both.ours, s.both.theirs, out);
          }
      }
    }
  return out.list;
}

export const textOf = (pieces: Piece[]) =>
  pieces.map((p) => ('agreed' in p ? p.agreed : p.clash.theirs)).join('');
const clashes = (pieces: Piece[]) => pieces.filter((p) => 'clash' in p).length;

// ---------------------------------------------------------------------------------- the fold

const nameKey = (name: string) =>
  name.normalize('NFKD').replace(/\p{M}/gu, '').toLowerCase().replace(/\s+/g, ' ').trim();

/** The units of an entity: a group's members together, or a field alone. */
function unitsOf(entity: Entity): Map<string, string[]> {
  const units = new Map<string, string[]>();
  for (const [field, def] of Object.entries(SCHEMA[entity])) {
    const key = def.group ?? field;
    units.set(key, [...(units.get(key) ?? []), field]);
  }
  return units;
}

const unitClass = (entity: Entity, members: string[]) =>
  (SCHEMA[entity][members[0] as string] as Def).cls;

/** Whether two values of a unit's member count as the same for a clash (Rust `essence`). */
const essence = (unit: string, member: string, value: Json): Json =>
  unit === 'deleted_at'
    ? value !== null
    : unit === 'state' && member === 'closed_at'
      ? null
      : value;

/** What the union of two logs comes to. A pure function of the records. */
export function fold(log: Rec[]): Outcome {
  const byDoc = new Map<string, Rec[]>();
  for (const r of [...log].sort(cmp)) {
    const key = `${r.entity}/${r.id}`;
    byDoc.set(key, [...(byDoc.get(key) ?? []), r]);
  }
  const outcome: Outcome = {
    docs: [],
    conflicts: [],
    deletedConflicts: [],
    values: [],
    deleteEdits: [],
    inbox: [],
    mergedLists: [],
  };
  const texts: TextClash[] = [];
  for (const recs of byDoc.values()) {
    const first = recs[0] as Rec;
    const doc: Doc = { entity: first.entity, id: first.id, values: {} };
    for (const [unit, members] of unitsOf(first.entity)) {
      const touching = recs.filter((r) => members.some((m) => m in r.fields));
      if (touching.length === 0) continue;
      const cls = unitClass(first.entity, members);
      if (cls === 'text') {
        const text = members.find((m) => (SCHEMA[first.entity][m] as Def).cls === 'text') as string;
        foldText(doc, touching, text, members, texts);
        continue;
      }
      let held: { values: Record<string, Json>; rec: Rec } | null = null;
      for (const r of touching) {
        const values: Record<string, Json> = { ...(held?.values ?? {}) };
        let beside = false;
        for (const m of members) {
          if (!(m in r.fields)) continue;
          // A group travels whole: a member the record did not change is what its author held, so
          // a different one standing means it had not seen the record before.
          const author = m in r.prev ? r.prev[m] : members.length > 1 ? r.fields[m] : undefined;
          if (held && author !== undefined && !same(author, values[m])) beside = true;
          values[m] = r.fields[m] as Json;
        }
        if (
          cls === 'value' &&
          first.entity !== 'setting' &&
          held &&
          beside &&
          members.some(
            (m) =>
              !same(essence(unit, m, held?.values[m] ?? null), essence(unit, m, values[m] ?? null)),
          )
        )
          outcome.values.push({
            entity: first.entity,
            id: first.id,
            unit,
            kept: values,
            keptDevice: r.stamp.device,
            replaced: held.values,
            replacedDevice: held.rec.stamp.device,
            at: r.stamp.ms,
          });
        held = { values, rec: r };
      }
      Object.assign(doc.values, held?.values);
    }
    outcome.docs.push(doc);
    // A record deleted by one device and edited by another without knowing of it: later by the
    // clock, or not yet received by the deleter (whichever clock is later).
    const deletes = recs.filter((r) => r.fields.deleted_at != null);
    const gone = deletes[deletes.length - 1];
    if (gone && doc.values.deleted_at != null) {
      const edit = recs.find(
        (r) =>
          r.stamp.device !== gone.stamp.device &&
          (cmp(r, gone) > 0 ||
            (gone.seen !== undefined && r.seq > (gone.seen[r.stamp.device] ?? 0))) &&
          Object.keys(r.fields).some((f) => f !== 'deleted_at'),
      );
      if (edit)
        outcome.deleteEdits.push({
          entity: first.entity,
          id: first.id,
          deleter: gone.stamp.device,
          editor: edit.stamp.device,
          at: edit.stamp.ms,
        });
    }
  }
  const live = new Set(
    outcome.docs.filter((d) => d.values.deleted_at == null).map((d) => `${d.entity}/${d.id}`),
  );
  outcome.conflicts = texts.filter((c) => live.has(`${c.entity}/${c.id}`));
  outcome.deletedConflicts = texts.filter((c) => !live.has(`${c.entity}/${c.id}`));
  rules(outcome, log);
  return outcome;
}

/** The replay of a text unit (crates/sync/src/texts.rs `replay`). */
function foldText(doc: Doc, recs: Rec[], text: string, members: string[], texts: TextClash[]) {
  let held: { values: Record<string, Json>; rec: Rec; at: number } | null = null;
  for (const r of recs) {
    const mine = Object.fromEntries(
      members.filter((m) => m in r.fields).map((m) => [m, r.fields[m] as Json]),
    );
    const theirs = String(r.fields[text] ?? '');
    const base = r.prev[text];
    const ours = String(held?.values[text] ?? '');
    if (held && typeof base === 'string' && base !== ours && theirs !== ours) {
      const pieces = textMerge(base, ours, theirs);
      if (clashes(pieces) > 0)
        texts.push({
          entity: doc.entity,
          id: doc.id,
          field: text,
          base,
          ours,
          oursDevice: held.rec.stamp.device,
          oursAt: held.rec.stamp.ms,
          theirs,
          theirsDevice: r.stamp.device,
          theirsAt: r.stamp.ms,
          pieces,
        });
      const merged = textOf(pieces);
      const values: Record<string, Json> = { [text]: merged };
      for (const m of members.filter((x) => x !== text)) {
        const [a, b] = [held.values[m] ?? null, mine[m] ?? null];
        values[m] =
          typeof a === 'boolean' && typeof b === 'boolean'
            ? a || b
            : merged === theirs
              ? b
              : typeof b === 'boolean'
                ? b
                : null;
      }
      held = { values, rec: r, at: r.stamp.ms };
    } else held = { values: mine, rec: r, at: r.stamp.ms };
  }
  Object.assign(doc.values, held?.values);
}

// ---------------------------------------------------------------------------------- the rules on top

/** Lists with one name merge into the older (the lower id, like Rust `list_sync::settle`); a Task whose List is gone goes to the Inbox. */
function rules(outcome: Outcome, log: Rec[]) {
  const lists = outcome.docs.filter((d) => d.entity === 'list' && d.values.deleted_at == null);
  const age = (a: Doc, b: Doc) => (a.id < b.id ? -1 : 1);
  const survivor = new Map<string, Doc>();
  for (const l of [...lists].sort(age)) {
    const key = nameKey(String(l.values.name));
    const kept = survivor.get(key);
    if (!kept) survivor.set(key, l);
    else {
      outcome.mergedLists.push({
        from: l.id,
        into: kept.id,
        fromDevice: log.find((r) => r.entity === 'list' && r.id === l.id)?.stamp.device ?? '',
        at: Number(l.values.created_at),
      });
      // The List that went away is deleted (Rust records that Delete), so it leaves the sidebar.
      l.values.deleted_at = Math.max(Number(l.values.created_at), Number(kept.values.created_at));
    }
  }
  const nameOf = new Map(
    outcome.docs.filter((d) => d.entity === 'list').map((d) => [d.id, String(d.values.name)]),
  );
  /** Where a Task naming `list` really lives: the List itself, the one it merged into, or the Inbox. */
  const home = (list: string): string | null => {
    const name = nameOf.get(list);
    return name === undefined ? null : (survivor.get(nameKey(name))?.id ?? null);
  };
  const lastKey = (list: string | null) =>
    outcome.docs
      .filter(
        (d) => d.entity === 'task' && d.values.list_id === list && d.values.deleted_at == null,
      )
      .reduce<string | null>(
        (max, d) =>
          max === null || String(d.values.sort_key) > max ? String(d.values.sort_key) : max,
        null,
      );
  const tasks = outcome.docs
    .filter((d) => d.entity === 'task' && d.values.deleted_at == null)
    .sort((a, b) => (String(a.values.sort_key) < String(b.values.sort_key) ? -1 : 1));
  for (const task of tasks) {
    const named = task.values.list_id as string | null;
    if (named === null) continue;
    const to = home(named);
    if (to === named) continue;
    const mover = [...log]
      .sort(cmp)
      .reverse()
      .find((r) => r.entity === 'task' && r.id === task.id && 'list_id' in r.fields);
    // The newest delete of the List says whose delete sent the Task here.
    const deleter = [...log]
      .sort(cmp)
      .reverse()
      .find((r) => r.entity === 'list' && r.id === named && r.fields.deleted_at != null);
    if (to === null)
      outcome.inbox.push({
        task: task.id,
        list: named,
        deleter: deleter?.stamp.device ?? '',
        at: mover?.stamp.ms ?? 0,
      });
    // A merged List's Tasks come after the survivor's own.
    const end = lastKey(to);
    task.values.list_id = to;
    if (to !== null && end !== null && String(task.values.sort_key) <= end)
      task.values.sort_key = keyAfter(end);
  }
}
