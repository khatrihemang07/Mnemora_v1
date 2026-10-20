<!--
  TitleBar: the integrated drag strip. macOS keeps its native traffic lights (overlay style,
  so the strip just leaves room for them); Windows and Linux get custom controls.
-->
<script lang="ts">
  import type { Snippet } from 'svelte';
  import { inTauriWindow, os } from '../lib/platform';
  import WindowControls from './WindowControls.svelte';

  interface Props {
    children?: Snippet;
    /** Content after the drag spacer, e.g. page actions. */
    trailing?: Snippet;
    controls?: boolean;
    /** Page top bar: 50px with a bottom hairline. */
    bar?: boolean;
  }
  let { children, trailing, controls = false, bar = false }: Props = $props();
  const showControls = $derived(controls && (os === 'windows' || os === 'linux') && inTauriWindow());
</script>

<div data-tauri-drag-region class="flex shrink-0 items-center {bar ? 'h-[50px] border-b border-line' : 'h-(--titlebar-h)'}">
  {@render children?.()}
  <div data-tauri-drag-region class="h-full flex-1"></div>
  {@render trailing?.()}
  {#if showControls}<WindowControls />{/if}
</div>
