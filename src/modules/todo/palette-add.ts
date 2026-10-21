import type { Parsed } from '../../lib/ipc/bindings';
import { queryClient } from '../../lib/ipc/query-client';
import { IpcError } from '../../lib/ipc/unwrap';
import { notify, notifyError } from '../../lib/ui/toast';
import { type CaptureFields, NO_WORDS, resolveFields } from './capture';
import { todoClock } from './clock.svelte';
import { captureSummary } from './counts';
import { todoPrefs } from './prefs.svelte';
import { createList, createTask, invalidateTodo, listsQuery, parseCapture } from './queries';
import { quickAddedMessage } from './tasks';
import { serializeUnderstood, understoodFromSpans } from './understood';

/** What ⌘K's "Add task" would do with the text: read like the capture box reads it. */
interface Reading {
  title: string;
  parsed: Parsed | null;
  fields: CaptureFields;
}

/** A Task from the palette starts in the Inbox with nothing, and the typed words set the rest. */
const INBOX = { listId: null, date: null, time: null, repeat: null, priority: 4 };

async function read(text: string): Promise<Reading | null> {
  const title = text.trim();
  if (!title) return null;
  const parsed = todoPrefs.parseDates ? await parseCapture(title, NO_WORDS) : null;
  return { title, parsed, fields: resolveFields(parsed, {}, INBOX) };
}

const listNameOf = async (id: string | null): Promise<string | null> => {
  if (id === null) return null;
  const sidebar = await queryClient.fetchQuery(listsQuery());
  return sidebar.lists.find((e) => e.list.id === id)?.list.name ?? null;
};

/**
 * The palette's second line for "Add task": the meaning it read from the text ("Tomorrow · 18:00
 * · P2"), or "Inbox" when the words say nothing. Never throws: a failed reading shows no line.
 */
export async function previewAddTask(text: string): Promise<string | null> {
  try {
    const reading = await read(text);
    if (!reading) return null;
    const list = await listNameOf(reading.fields.listId);
    return captureSummary(reading.fields, list, todoClock.today, todoPrefs.timeFormat);
  } catch {
    return null;
  }
}

/** Adds the Task the text describes, exactly as the capture box would, and says where it went. */
export async function addTaskFromPalette(text: string): Promise<void> {
  try {
    const reading = await read(text);
    if (!reading) return;
    const { title, parsed, fields } = reading;
    const target = fields.newList === null ? fields.listId : (await createList(fields.newList)).id;
    const task = await createTask({
      title,
      listId: target,
      date: fields.date,
      time: fields.time,
      repeat: fields.repeat,
      priority: fields.priority,
      understood: serializeUnderstood(
        understoodFromSpans(parsed?.spans ?? [], (s) => (s.kind === 'list' ? target : s.value)),
      ),
    });
    await invalidateTodo();
    const where = fields.newList ?? (await listNameOf(target)) ?? 'Inbox';
    notify(quickAddedMessage(task.title, where));
  } catch (e) {
    notifyError(e instanceof IpcError ? e.message : "Couldn't add the Task");
  }
}
