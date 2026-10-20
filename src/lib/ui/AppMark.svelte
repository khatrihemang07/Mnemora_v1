<!-- AppMark: the A4 "Memory Spiral" app icon as an inline mark. Same art as src-tauri/icons/source/app-icon-fullbleed.svg. -->
<script lang="ts">
  let { class: cls = 'size-[22px]' }: { class?: string } = $props();
</script>

<svg viewBox="0 0 1024 1024" aria-hidden="true" class="shrink-0 {cls}">
  <defs>
    <linearGradient id="mnemora-mark-g" x1="0" y1="1" x2="1" y2="0">
      <stop offset="0" stop-color="#ff7a59" />
      <stop offset="1" stop-color="#ffc94d" />
    </linearGradient>
  </defs>
  <rect width="1024" height="1024" rx="230" fill="url(#mnemora-mark-g)" />
  <path
    transform="translate(52 52) scale(9.2)"
    d="M46 53.5A11 11 0 0 0 68 53.5A18 18 0 0 0 32 53.5A25 25 0 0 0 82 53.5A32 32 0 0 0 18 53.5"
    fill="none"
    stroke="#2a1608"
    stroke-width="6.5"
    stroke-linecap="round"
  />
</svg>
