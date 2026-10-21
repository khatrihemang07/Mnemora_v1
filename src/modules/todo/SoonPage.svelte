<!-- SoonPage: a view whose screen arrives later (Done). It still routes and shows active. -->
<script lang="ts">
  import EmptyState from '../../lib/ui/EmptyState.svelte';
  import PageHeader from '../../lib/ui/PageHeader.svelte';
  import { VIEW_ICONS } from './view-icons';
  import { type SmartViewId, viewLabel } from './views';

  let { id }: { id: SmartViewId } = $props();

  const WHY: Partial<Record<SmartViewId, string>> = {
    done: 'What you have finished will be listed here by day.',
  };
</script>

<PageHeader title={viewLabel(id)} icon={VIEW_ICONS[id]} hue="--hue-todo" />
<div class="mx-auto max-w-3xl">
  <EmptyState icon={VIEW_ICONS[id]} hue="--hue-todo" title="{viewLabel(id)} is on its way" description={WHY[id]} />
</div>
