import { beforeEach, describe, expect, it, vi } from 'vitest';
import fixture from '../../lib/ipc/fixtures/todo-links.json';

// The Todo widget's Kotlin (WidgetLinks, JUnit) builds the `canonical` links from the same file.

const params = vi.hoisted(() => ({ task: null as string | null, deleted: [] as string[] }));
vi.mock('../../lib/router/url-param', () => ({
  urlParam: {
    get: () => params.task,
    delete: (name: string) => {
      params.deleted.push(name);
      params.task = null;
    },
  },
}));

import { openTodoLink, parseTodoLink } from './deep-link';
import { todoNav } from './nav.svelte';
import { DEFAULT_VIEW } from './views';

beforeEach(() => {
  params.task = null;
  params.deleted = [];
  todoNav.go(DEFAULT_VIEW);
});

describe('todo-links fixture, parsed', () => {
  it.each([...fixture.canonical, ...fixture.parse_only])('$name', ({ url, view, task }) => {
    expect(parseTodoLink(new URL(url))).toEqual({ view, task });
  });
});

describe('openTodoLink', () => {
  it('shows the view and goes to Todo', () => {
    expect(openTodoLink(new URL('mnemora://todo?view=upcoming'))).toBe('/todo');
    expect(todoNav.view).toEqual({ kind: 'smart', id: 'upcoming' });
  });

  it('shows a List by its id', () => {
    openTodoLink(new URL('mnemora://todo?view=list:L1'));
    expect(todoNav.view).toEqual({ kind: 'list', id: 'L1' });
  });

  it('opens a Task in its sheet over the view the link names', () => {
    const path = openTodoLink(new URL('mnemora://todo?view=inbox&task=a%20b'));
    expect(path).toBe('/todo?task=a%20b');
    expect(todoNav.view).toEqual({ kind: 'smart', id: 'inbox' });
  });

  it('leaves the view alone when the link names none', () => {
    todoNav.go({ kind: 'smart', id: 'all' });
    expect(openTodoLink(new URL('mnemora://todo?task=T1'))).toBe('/todo?task=T1');
    expect(todoNav.view).toEqual({ kind: 'smart', id: 'all' });
  });

  it('closes an open Task sheet for a link to a view', () => {
    params.task = 'T1';
    expect(openTodoLink(new URL('mnemora://todo?view=today'))).toBe('/todo');
    expect(params.deleted).toEqual(['task']);
  });
});
