<!-- CountdownRing: how much of a window is left, as a ring that empties; pass 1 and `still` for a ring that only marks "waiting". -->
<script lang="ts">
  interface Props {
    /** 0 to 1: the share of the window left. */
    fraction: number;
    size?: number;
    /** No sweep: a plain ring, for "waiting" with no countdown. */
    still?: boolean;
  }
  let { fraction, size = 20, still = false }: Props = $props();

  const STROKE = 3;
  const radius = $derived((size - STROKE) / 2);
  const circumference = $derived(2 * Math.PI * radius);
</script>

<svg
  width={size}
  height={size}
  viewBox="0 0 {size} {size}"
  aria-hidden="true"
  class="shrink-0 -rotate-90 text-hue-sync {still ? 'animate-spin [animation-duration:1.2s]' : ''}"
>
  <circle cx={size / 2} cy={size / 2} r={radius} fill="none" stroke-width={STROKE} class="stroke-line" />
  <circle
    cx={size / 2}
    cy={size / 2}
    r={radius}
    fill="none"
    stroke="currentColor"
    stroke-width={STROKE}
    stroke-linecap="round"
    stroke-dasharray={circumference}
    stroke-dashoffset={still ? circumference * 0.7 : circumference * (1 - fraction)}
  />
</svg>
