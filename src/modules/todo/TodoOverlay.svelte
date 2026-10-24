<!-- TodoOverlay: what the shell keeps mounted while Todo is on: the floating Quick add, ⌘K's List actions and the follower that moves you off a List Sync merged away and the one that closes the sheet of a Task deleted elsewhere. -->
<script lang="ts">
  import ListMergeFollower from './ListMergeFollower.svelte';
  import PaletteLists from './PaletteLists.svelte';
  import QuickAddHost from './QuickAddHost.svelte';
  import RemoteDeleteFollower from './RemoteDeleteFollower.svelte';
</script>

<QuickAddHost />
<ListMergeFollower />
<RemoteDeleteFollower />
<PaletteLists />
