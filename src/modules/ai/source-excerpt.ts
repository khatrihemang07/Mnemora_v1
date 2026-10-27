import { commands, type Page, type Source, type Task } from '../../lib/ipc/bindings';
import { unwrap } from '../../lib/ipc/unwrap';

/** What a Source preview shows of the item it cites. */
export interface SourceExcerpt {
  title: string;
  date: string | null;
  /** The first lines of the body (an Entry's text, a Task's notes); empty when there are none. */
  excerpt: string;
}

/** The reads the preview uses: the Journal's Page feed and one Task. Injected so tests need no IPC. */
export interface SourceReader {
  /** The Page starting at `date` (the Page's own Entries included), newest first. */
  journalPage(date: string): Promise<Page[]>;
  todoTask(id: string): Promise<Task>;
}

const liveReader: SourceReader = {
  journalPage: (date) => unwrap(commands.journalPages(date, 1, 'from')),
  todoTask: (id) => unwrap(commands.todoTask(id)),
};

const LINES = 3;
const CHARACTERS = 240;

/** The first few non-empty lines of a body, cut short for a small preview. */
export function firstLines(body: string): string {
  const kept = body
    .split('\n')
    .map((line) => line.trimEnd())
    .filter((line) => line.trim() !== '')
    .slice(0, LINES)
    .join('\n');
  return kept.length > CHARACTERS ? `${kept.slice(0, CHARACTERS).trimEnd()}…` : kept;
}

/**
 * Loads what a Source preview shows through the Journal and Todo reads that already exist: no field
 * is added to a Source. A missing Entry or Task rejects with the Module's own "no longer exists" wording.
 */
export async function loadSource(
  source: Source,
  read: SourceReader = liveReader,
): Promise<SourceExcerpt> {
  if (source.module === 'todo') {
    const task = await read.todoTask(source.id);
    return { title: source.title, date: task.due_date, excerpt: firstLines(task.notes) };
  }
  if (source.date === null) throw new Error('That Entry no longer exists');
  const pages = await read.journalPage(source.date);
  const entry = pages
    .flatMap((page) => page.entries)
    .find((candidate) => candidate.id === source.id);
  if (!entry) throw new Error('That Entry no longer exists');
  return { title: source.title, date: entry.page_date, excerpt: firstLines(entry.body) };
}
