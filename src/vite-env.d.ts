/// <reference types="svelte" />
/// <reference types="vite/client" />
/// <reference types="unplugin-icons/types/svelte" />

interface Window {
  /** Injected by the Android MainActivity; undefined on desktop and in the browser. */
  MnemoraAndroid?: import('./lib/theme/system-bars').AndroidBridge;
}
