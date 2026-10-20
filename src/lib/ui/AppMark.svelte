<!-- AppMark: the A4 "Memory Spiral" app icon as an inline mark. Same paths as src-tauri/icons/source/app-icon.svg. -->
<script lang="ts">
  let { class: cls = 'size-[22px]' }: { class?: string } = $props();
</script>

<svg viewBox="0 0 128 128" aria-hidden="true" class="shrink-0 {cls}">
  <defs>
    <linearGradient id="mnemora-mark-g" x1="0" y1="1" x2="1" y2="0">
      <stop offset="0" stop-color="#ff7a59" />
      <stop offset="1" stop-color="#ffc94d" />
    </linearGradient>
  </defs>
  <path d="M64 0c45 0 64 19 64 64s-19 64-64 64S0 109 0 64 19 0 64 0z" fill="url(#mnemora-mark-g)" />
  <path
    d="M66 64a4 4 0 1 1-8 0 10 10 0 0 1 18 0c0 11-9 18-20 18-14 0-24-11-24-24 0-17 14-30 32-30 21 0 36 16 36 36"
    fill="none"
    stroke="#2a1608"
    stroke-width="7"
    stroke-linecap="round"
  />
</svg>
