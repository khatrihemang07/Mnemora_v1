<!-- RouterOutlet: renders the current route with a 120ms fade between routes (N14-B).
  Navigation through the router wrapper uses a View Transition; this keyframe fade is the fallback. -->
<script lang="ts">
  import { Router } from 'sv-router';
  let { pathname }: { pathname: string } = $props();
</script>

{#key pathname}
  <div class="route-fade h-full">
    <Router />
  </div>
{/key}
