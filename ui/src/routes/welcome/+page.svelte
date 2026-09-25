<script lang="ts">
  import { goto } from '$app/navigation';
  import { ArrowRight, FileArchive, KeyRound, LockKeyhole, Server } from 'lucide-svelte';
  import { commands, type SourceSummary } from '$lib/bindings';
  import Brand from '$lib/components/Brand.svelte';

  let sources = $state<SourceSummary[]>([]);

  $effect(() => {
    commands.listSources().then((result) => {
      if (result.status === 'ok') sources = result.data;
    });
  });
</script>

<!-- S01 — Welcome / source picker. This build offers the IMAP path only. -->
<main>
  <header>
    <Brand />
    <h1>Find everything your inbox<br />is costing you. Then end it.</h1>
    <p class="lede">
      Subscriptions, newsletters, forgotten free trials — scanned, priced, and terminated from one
      place.
    </p>
  </header>

  <div class="cards">
    <div class="card off" aria-disabled="true">
      <div class="icon"><FileArchive size={22} strokeWidth={2.75} /></div>
      <h2>Import an mbox file</h2>
      <p>Drop an export from any mail app. Fastest, fully offline.</p>
      <span class="cta">Coming in a later version</span>
    </div>

    <button type="button" class="card" onclick={() => goto('/connect')}>
      <div class="icon"><Server size={22} strokeWidth={2.75} /></div>
      <h2>Connect via IMAP</h2>
      <p>Any mailbox, with an app password. Stays in sync.</p>
      <span class="cta">Connect <ArrowRight size={15} strokeWidth={2.75} /></span>
    </button>

    <div class="card off" aria-disabled="true">
      <span class="tag">guided setup</span>
      <div class="icon"><KeyRound size={22} strokeWidth={2.75} /></div>
      <h2>Connect Gmail</h2>
      <p>Bring your own OAuth client — we walk you through it.</p>
      <span class="cta">Coming in a later version</span>
    </div>
  </div>

  {#if sources.length > 0}
    <button type="button" class="home" onclick={() => goto('/home')}>
      Back to the dashboard <ArrowRight size={15} strokeWidth={2.75} />
    </button>
    <ul class="sources">
      {#each sources as source (source.id)}
        <li>
          <span>{source.label}</span>
          <button type="button" onclick={() => goto(`/scan?source=${source.id}`)}>
            Scan again <ArrowRight size={14} strokeWidth={2.75} />
          </button>
        </li>
      {/each}
    </ul>
  {/if}

  <footer>
    <LockKeyhole size={16} strokeWidth={2.75} />
    No account. No cloud. Your mail is analyzed on this computer and never leaves it.
  </footer>
</main>

<style>
  main {
    min-height: 100vh;
    display: flex;
    flex-direction: column;
    align-items: center;
    padding: 5.25rem var(--space-8) var(--space-8);
    box-sizing: border-box;
  }

  header {
    display: flex;
    flex-direction: column;
    align-items: center;
    text-align: center;
  }

  h1 {
    margin: 1.875rem 0 0;
    font-family: var(--font-heading);
    font-weight: var(--font-heading-weight);
    font-size: 2.875rem;
    line-height: 1.12;
    color: var(--hd);
  }

  .lede {
    margin: var(--space-4) 0 0;
    max-width: 35rem;
    font-size: 1.03rem;
    color: var(--mut);
  }

  .cards {
    display: flex;
    flex-wrap: wrap;
    justify-content: center;
    gap: 1.25rem;
    margin-top: 3.25rem;
  }

  .card {
    position: relative;
    width: 20.625rem;
    box-sizing: border-box;
    text-align: left;
    font: inherit;
    color: inherit;
    background: var(--card);
    border: var(--stroke) solid var(--line);
    border-radius: var(--radius-lg);
    padding: 1.625rem;
    box-shadow: var(--shadow-sm);
    transition:
      box-shadow 0.15s,
      transform 0.15s;
  }

  button.card {
    cursor: pointer;
  }

  button.card:hover {
    box-shadow: var(--shadow-md);
    transform: translateY(-0.125rem);
  }

  button.card:active {
    transform: none;
    background: var(--card2);
  }

  .card.off {
    opacity: 0.45;
  }

  .icon {
    width: 2.75rem;
    height: 2.75rem;
    border-radius: 0.875rem;
    background: var(--sageSoft);
    color: var(--sageDeep);
    display: flex;
    align-items: center;
    justify-content: center;
  }

  h2 {
    margin: var(--space-4) 0 0;
    font-family: var(--font-heading);
    font-weight: var(--font-heading-weight);
    font-size: 1.1875rem;
    color: var(--hd);
  }

  .card p {
    margin: var(--space-1) 0 0;
    font-size: 0.875rem;
    line-height: 1.5;
    color: var(--mut);
  }

  .cta {
    margin-top: var(--space-4);
    display: flex;
    align-items: center;
    gap: var(--space-1);
    font-size: 0.84rem;
    font-weight: 700;
    color: var(--accDeep);
  }

  .tag {
    position: absolute;
    top: 1.125rem;
    right: 1.125rem;
    font-size: 0.6875rem;
    font-weight: 700;
    background: var(--card2);
    color: var(--mut);
    padding: 0.1875rem 0.625rem;
    border-radius: var(--pill);
  }

  .home {
    display: flex;
    align-items: center;
    gap: var(--space-1);
    margin-top: var(--space-8);
    border: none;
    background: none;
    font: inherit;
    font-size: 0.9rem;
    font-weight: 700;
    color: var(--accDeep);
    cursor: pointer;
  }

  .sources {
    list-style: none;
    margin: var(--space-4) 0 var(--space-8);
    padding: 0;
    width: min(100%, 40rem);
  }

  .sources li {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--space-3);
    padding: var(--space-3) var(--space-4);
    background: var(--card);
    border: var(--stroke) solid var(--line);
    border-radius: var(--radius-md);
    font-size: 0.875rem;
  }

  .sources li + li {
    margin-top: var(--space-2);
  }

  .sources button {
    display: flex;
    align-items: center;
    gap: var(--space-1);
    border: none;
    background: none;
    font: inherit;
    font-weight: 700;
    color: var(--accDeep);
    cursor: pointer;
  }

  footer {
    margin-top: auto;
    background: var(--card2);
    padding: var(--space-2) 1.25rem;
    border-radius: var(--pill);
    display: flex;
    align-items: center;
    gap: var(--space-2);
    font-size: 0.84rem;
    color: var(--mut);
  }

  footer :global(svg) {
    flex: none;
  }
</style>
