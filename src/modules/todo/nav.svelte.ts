import { DEFAULT_VIEW, type TodoView } from './views';

/**
 * The view Todo is showing. Todo is one route, so the view is state here, kept for the session:
 * a fresh launch opens Today, coming back from the Journal keeps the place.
 */
class TodoNav {
  view = $state.raw<TodoView>(DEFAULT_VIEW);

  go(view: TodoView) {
    this.view = view;
  }
}

export const todoNav = new TodoNav();
