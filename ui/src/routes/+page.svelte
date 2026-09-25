<script lang="ts">
  import { commands, type StoreStatus } from '$lib/bindings';

  let status = $state<StoreStatus | null>(null);

  $effect(() => {
    commands.storeStatus().then((result) => (status = result));
  });
</script>

<main>
  <h1>EmailTerminator</h1>
  {#if status?.state === 'open'}
    <p>Local data is encrypted. The key is in the {status.keyBackend}.</p>
  {:else if status?.state === 'locked'}
    <p>The database key no longer opens your local data.</p>
  {:else if status?.state === 'failed'}
    <p>Local data could not open: {status.message}</p>
  {/if}
</main>

<style>
  main {
    padding: var(--space-8);
  }

  h1 {
    margin: 0 0 var(--space-3);
    color: var(--hd);
    font-family: var(--font-heading);
    font-weight: var(--font-heading-weight);
  }

  p {
    margin: 0;
    color: var(--mut);
  }
</style>
