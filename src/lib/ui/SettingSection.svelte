<!-- SettingSection: a titled group of SettingRows inside a Card. -->
<script lang="ts">
  import type { Snippet } from 'svelte';
  import Card from './Card.svelte';
  interface Props {
    title: string;
    description?: string;
    children: Snippet;
  }
  let { title, description, children }: Props = $props();
</script>

<section class="mb-6">
  <h2 class="font-display text-lg" style="font-weight: var(--display-weight); letter-spacing: var(--display-track)">
    {title}
  </h2>
  {#if description}<p class="mt-0.5 text-muted">{description}</p>{/if}
  <Card padded={false} class="mt-3 divide-y divide-line">
    {@render children()}
  </Card>
</section>
