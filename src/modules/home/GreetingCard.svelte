<!-- GreetingCard: S1 hero gradient (home → journal → ai hues) with a time-of-day greeting. -->
<script lang="ts">
  import { formatDate } from '../../lib/time/format';
  import type { DateFormat } from '../../lib/time/format';
  import { greetingText } from '../../lib/time/greeting';

  interface Props {
    now: Date;
    name: string;
    dateFormat: DateFormat;
  }
  let { now, name, dateFormat }: Props = $props();
</script>

<section
  class="rounded-lg p-[calc(var(--pad)*1.6)] text-hero-ink"
  style="background: linear-gradient(125deg, var(--hero-from) 0%, var(--hero-via) 48%, var(--hero-to) 100%)"
>
  <h1
    class="font-display text-3xl leading-tight"
    style="font-weight: var(--display-weight); letter-spacing: var(--display-track)"
  >
    {greetingText(now, name)}
  </h1>
  <p class="mt-1 text-hero-ink-muted">{formatDate(now, dateFormat)}</p>
</section>
