<script lang="ts">
  import { goto } from '$app/navigation';
  import { KeyRound, TriangleAlert } from 'lucide-svelte';
  import { commands, type StoreStatus } from '$lib/bindings';
  import Brand from '$lib/components/Brand.svelte';

  // Opens the dashboard, or S01 on a fresh install, or says why the local
  // data cannot open (S16).
  let status = $state<StoreStatus | null>(null);
  let erasing = $state(false);
  let problem = $state<string | null>(null);

  $effect(() => {
    commands.storeStatus().then(async (s) => {
      status = s;
      if (s.state !== 'open') return;
      const sources = await commands.listSources();
      const any = sources.status === 'ok' && sources.data.length > 0;
      await goto(any ? '/home' : '/welcome', { replaceState: true });
    });
  });

  async function startFresh() {
    erasing = true;
    const result = await commands.eraseLocalData();
    // Success restarts the app; only a refusal returns.
    if (result.status === 'error') problem = result.error;
    erasing = false;
  }
</script>

{#if status && status.state !== 'open'}
  <!-- S16 — Database key missing, or the data failed to open -->
  <main>
    <Brand size="small" />
    <section class="card">
      {#if status.state === 'locked'}
        <h1><KeyRound size={17} strokeWidth={2.75} /> The key to your local data is gone</h1>
        <p>
          The OS keychain no longer holds the key that encrypts EmailTerminator's data, so it cannot
          be opened. Your mailbox is untouched: a fresh scan reads it again from the start.
        </p>
        <div class="actions">
          <button type="button" class="dark" disabled={erasing} onclick={startFresh}>
            Start fresh and scan again
          </button>
        </div>
      {:else}
        <h1><TriangleAlert size={17} strokeWidth={2.75} /> The local data could not open</h1>
        <p>{status.message}</p>
        <p>Your mailbox is untouched.</p>
      {/if}
      {#if problem}<p role="alert">{problem}</p>{/if}
    </section>
  </main>
{/if}

<style>
  main {
    min-height: 100vh;
    box-sizing: border-box;
    padding: 1.75rem 3rem;
    display: flex;
    flex-direction: column;
    gap: 5rem;
  }

  .card {
    align-self: center;
    max-width: 30rem;
    background: var(--card);
    border: var(--stroke) solid var(--line);
    border-radius: var(--radius-lg);
    padding: 1.625rem;
  }

  h1 {
    display: flex;
    align-items: center;
    gap: 0.625rem;
    margin: 0;
    font-size: 0.875rem;
    font-weight: 800;
    color: var(--accDeep);
  }

  p {
    margin: var(--space-2) 0 0;
    font-size: 0.8125rem;
    line-height: 1.55;
    color: var(--mut);
  }

  .actions {
    display: flex;
    gap: var(--space-2);
    margin-top: 0.875rem;
  }

  .dark {
    border: none;
    cursor: pointer;
    background: var(--fg);
    color: var(--bg);
    font-family: var(--font-body);
    font-size: 0.78rem;
    font-weight: 700;
    padding: var(--space-2) 1rem;
    border-radius: var(--pill);
  }

  .dark:hover {
    filter: brightness(1.2);
  }

  .dark:disabled {
    opacity: 0.45;
    cursor: default;
  }
</style>
