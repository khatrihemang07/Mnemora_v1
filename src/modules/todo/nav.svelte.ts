import { DEFAULT_VIEW, type TodoView } from './views';

/**
 * The view Todo is showing. Todo is one route, so the view is state here, kept for the session:
 * a fresh launch opens Today, coming back from the Journal keeps the place.
 */
class TodoNav {
  view = $state.raw<TodoView>(DEFAULT_VIEW);

  /** Bumped to ask the page to scroll to its top (the phone's Todo tab, tapped again). */
  topRequests = $state(0);
  /** Which way the last swipe went, so the new view slides in from that side; none otherwise. */
  slide = $state<'next' | 'prev' | null>(null);

  go(view: TodoView) {
    this.view = view;
    this.slide = null;
  }

  /** A swipe moved to `view`: `step` 1 is towards the next segment. */
  swipeTo(view: TodoView, step: -1 | 1) {
    this.view = view;
    this.slide = step === 1 ? 'next' : 'prev';
  }

  /** Back to Today at the top of the page: the Todo tab tapped while already in Todo. */
  home() {
    this.go(DEFAULT_VIEW);
    this.topRequests += 1;
  }
}

export const todoNav = new TodoNav();
