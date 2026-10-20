import { mount } from 'svelte';
import './app.css';
import App from './App.svelte';
import { installGlobalErrorHandlers } from './lib/ipc/errors';

async function boot() {
  // Dev-only: in a plain browser there is no Rust side, so a mock stands in for it.
  if (import.meta.env.DEV && !('__TAURI_INTERNALS__' in window)) {
    await (await import('./lib/ipc/mock')).installMock();
  }
  installGlobalErrorHandlers();

  const target = document.getElementById('app');
  if (!target) throw new Error('#app not found');
  mount(App, { target });
}

void boot();
