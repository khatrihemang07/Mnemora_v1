<!--
  ProgressBar: a hairline bar for how far something has got (`value` from 0 to 1). `visible` fades it in
  and out (the caller decides when: a sync's row shows it only if the sync outlasts 600ms); the fill moves
  with the usual motion token, so it jumps under Reduce motion. `hue` is a Module hue token name.
-->
<script lang="ts">
  interface Props {
    value: number;
    label: string;
    visible?: boolean;
    hue?: string;
    class?: string;
  }
  let { value, label, visible = true, hue = '--accent', class: cls = '' }: Props = $props();

  const percent = $derived(Math.round(Math.min(1, Math.max(0, value)) * 100));
</script>

<div
  role="progressbar"
  aria-label={label}
  aria-valuemin={0}
  aria-valuemax={100}
  aria-valuenow={percent}
  aria-hidden={!visible}
  style="--bar-hue: var({hue})"
  class="h-[3px] overflow-hidden rounded-full bg-line transition-opacity duration-(--dur-fade) {visible
    ? 'opacity-100'
    : 'opacity-0'} {cls}"
>
  <div
    class="h-full rounded-full bg-(--bar-hue) transition-[width] duration-(--dur) ease-out"
    style="width: {percent}%"
  ></div>
</div>
