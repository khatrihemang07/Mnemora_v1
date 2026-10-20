import { svelte } from '@sveltejs/vite-plugin-svelte';
import tailwindcss from '@tailwindcss/vite';
import Icons from 'unplugin-icons/vite';
import { defineConfig } from 'vite';

// Set by `cargo tauri android dev` / iOS dev so the device can reach the dev server.
const host = process.env.TAURI_DEV_HOST;

export default defineConfig({
  plugins: [tailwindcss(), Icons({ compiler: 'svelte' }), svelte()],
  // Keep Rust compiler errors visible in the terminal.
  clearScreen: false,
  envPrefix: ['VITE_', 'TAURI_'],
  server: {
    port: 47821,
    strictPort: true,
    host: host || false,
    hmr: host ? { protocol: 'ws', host, port: 47822 } : undefined,
    watch: { ignored: ['**/src-tauri/**', '**/crates/**', '**/target/**'] },
  },
  build: {
    target: 'safari15',
    sourcemap: false,
  },
});
