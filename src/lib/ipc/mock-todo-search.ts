/**
 * Browser-mock Task search. Follows crates/todo/src/search.rs with plain scans instead of an
 * index, reusing the Journal mock's matching (word prefixes, case and accents ignored, quoted
 * phrases, typos as a separate group). The shared fixtures (fixtures/todo-search-rules.json) keep
 * it in step with Rust.
 */
import type {
  Comment,
  Task,
  TaskHit,
  TaskSearchResults,
  SnippetSource,
  Span,
} from './bindings';
import {
  hasMarks,
  highlightSpans,
  matches,
  nearWords,
  parse,
  plainText,
  type Near,
  type Term,
  words,
  windowSpans,
} from './mock-journal-search';

const MAX_EXACT = 500;
const MAX_SIMILAR = 100;

/** Open Tasks first in manual order, then closed ones, latest closed first. */
function byResultOrder(a: Task, b: Task): number {
  const aOpen = a.state === 'open';
  if (aOpen !== (b.state === 'open')) return aOpen ? -1 : 1;
  if (aOpen) return a.sort_key < b.sort_key ? -1 : a.sort_key > b.sort_key ? 1 : cmpId(a, b);
  return (b.closed_at ?? 0) - (a.closed_at ?? 0) || cmpId(a, b);
}
const cmpId = (a: Task, b: Task) => (a.id < b.id ? -1 : a.id > b.id ? 1 : 0);

function hit(task: Task, own: Comment[], terms: Term[], near: Near): TaskHit {
  let line: string | null = null;
  let source: SnippetSource | null = null;
  for (const l of task.notes.split('\n')) {
    const plain = plainText(l);
    if (hasMarks(plain, terms, near)) {
      line = plain;
      source = 'notes';
      break;
    }
  }
  if (line === null) {
    for (const c of own) {
      const plain = plainText(c.body);
      if (hasMarks(plain, terms, near)) {
        line = plain;
        source = 'comment';
        break;
      }
    }
  }
  const snippet: Span[] = line === null ? [] : windowSpans(line, terms, near);
  return {
    task: JSON.parse(JSON.stringify(task)) as Task,
    title: highlightSpans(task.title, terms, near),
    snippet,
    snippet_source: source,
  };
}

/** Searches `live` Tasks and the live `comments` on them. */
export function searchTasks(live: Task[], comments: Comment[], query: string): TaskSearchResults {
  const terms = parse(query);
  const empty: TaskSearchResults = {
    exact: [],
    similar: [],
    truncated: false,
    similar_truncated: false,
  };
  if (terms.length === 0) return empty;

  const commentsOf = new Map<string, Comment[]>();
  for (const c of comments) commentsOf.set(c.task_id, [...(commentsOf.get(c.task_id) ?? []), c]);
  // A Task is one row (title, then notes: a phrase never spans them); each Comment is another.
  const rows = live.map((t) => ({
    task: t,
    own: commentsOf.get(t.id) ?? [],
    rows: [[...words(t.title), '\0', ...words(t.notes)], ...(commentsOf.get(t.id) ?? []).map((c) => words(c.body))],
  }));
  const vocab = new Map<string, number>();
  for (const r of rows) {
    for (const tokens of r.rows) {
      for (const tok of new Set(tokens)) vocab.set(tok, (vocab.get(tok) ?? 0) + 1);
    }
  }
  const sorted = rows.sort((a, b) => byResultOrder(a.task, b.task));
  const found = (near: Near) => sorted.filter((r) => r.rows.some((tokens) => matches(tokens, terms, near)));

  const exact = found([]);
  const exactIds = new Set(exact.map((r) => r.task.id));
  const near = nearWords(terms, vocab);
  const similar = near.some((n) => n.size > 0)
    ? found(near).filter((r) => !exactIds.has(r.task.id))
    : [];
  return {
    exact: exact.slice(0, MAX_EXACT).map((r) => hit(r.task, r.own, terms, [])),
    similar: similar.slice(0, MAX_SIMILAR).map((r) => hit(r.task, r.own, terms, near)),
    truncated: exact.length > MAX_EXACT,
    similar_truncated: similar.length > MAX_SIMILAR,
  };
}
