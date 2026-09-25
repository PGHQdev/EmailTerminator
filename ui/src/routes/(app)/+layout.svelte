<script lang="ts">
  import Palette from '$lib/components/Palette.svelte';
  import Rail from '$lib/components/Rail.svelte';

  let { children } = $props();
  let palette = $state(false);

  function onkeydown(event: KeyboardEvent) {
    if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === 'k') {
      event.preventDefault();
      palette = !palette;
    }
  }
</script>

<svelte:window {onkeydown} />

<Rail />
<div class="screen">
  {@render children()}
</div>
<Palette bind:open={palette} />

<style>
  .screen {
    min-height: 100vh;
    box-sizing: border-box;
    padding: 1.625rem 3rem 2.25rem 7.625rem;
  }
</style>
