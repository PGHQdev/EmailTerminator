<script lang="ts">
  import { Clock } from 'lucide-svelte';
  import Empty from '$lib/components/Empty.svelte';
</script>

<!-- S14 — Activity log. The log itself arrives with actions in M3; until the
     app acts, the empty state (S16) is the whole screen. -->
<h1>Activity</h1>
<Empty title="No actions yet" muted>
  {#snippet icon()}<Clock size={26} strokeWidth={2.75} />{/snippet}
  Everything the app does on your behalf shows up here, with evidence.
</Empty>

<style>
  h1 {
    margin: 0;
    font-family: var(--font-heading);
    font-weight: var(--font-heading-weight);
    font-size: 1.875rem;
    color: var(--hd);
  }
</style>
