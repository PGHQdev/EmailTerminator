<script lang="ts">
  import { goto } from '$app/navigation';
  import { ArrowLeft, LoaderCircle } from 'lucide-svelte';
  import { commands, type ConnectError } from '$lib/bindings';
  import Brand from '$lib/components/Brand.svelte';
  import SignInFailed from '$lib/components/SignInFailed.svelte';

  // S01's Outlook path: no mockup; it follows the IMAP connect form and S16's
  // error. The browser does the sign-in; this page waits for it.
  let waiting = $state(false);
  let error = $state<ConnectError | null>(null);

  async function signIn() {
    waiting = true;
    error = null;
    const result = await commands.addOutlookSource();
    waiting = false;
    if (result.status === 'ok') await goto(`/scan?source=${result.data.id}`);
    else if (result.error.kind === 'cancelled') await goto('/');
    else error = result.error;
  }

  function leave() {
    if (waiting) commands.cancelOutlookSignIn();
    else goto('/');
  }

  $effect(() => {
    signIn();
  });
</script>

<main>
  <header>
    <button type="button" class="back" onclick={leave}>
      <ArrowLeft size={16} strokeWidth={2.75} /> Back
    </button>
    <Brand size="small" />
  </header>

  <section>
    <h1>Sign in to Outlook.com</h1>
    <p class="lede">
      Outlook, Hotmail and Live mailboxes. Microsoft no longer accepts app passwords for them, so you
      sign in with Microsoft instead. Only the sign-in token is kept, in this computer's keychain.
    </p>

    {#if error?.kind === 'signInRefused'}
      <SignInFailed
        host={error.host}
        serverSays={error.serverSays}
        guide={null}
        onRetry={signIn}
        oauth
      />
    {:else if error}
      <p class="problem" role="alert">{'message' in error ? error.message : ''}</p>
      <button type="button" class="primary" onclick={signIn}>Try again</button>
    {:else if waiting}
      <div class="waiting" role="status">
        <LoaderCircle size={18} strokeWidth={2.75} />
        Finish the sign-in in your browser. This page continues on its own.
      </div>
      <button type="button" class="secondary" onclick={leave}>Cancel</button>
    {/if}
  </section>
</main>

<style>
  main {
    min-height: 100vh;
    padding: 1.75rem 3rem;
    box-sizing: border-box;
  }

  header {
    display: flex;
    align-items: center;
    gap: var(--space-4);
  }

  .back {
    display: flex;
    align-items: center;
    gap: var(--space-1);
    border: none;
    background: none;
    font: inherit;
    font-size: 0.875rem;
    font-weight: 600;
    color: var(--mut);
    cursor: pointer;
  }

  .back:hover {
    color: var(--fg);
  }

  section {
    width: min(100%, 30rem);
    margin: 3.5rem 0 0 var(--space-8);
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: var(--space-4);
  }

  h1 {
    margin: 0;
    font-family: var(--font-heading);
    font-weight: var(--font-heading-weight);
    font-size: 2rem;
    color: var(--hd);
  }

  .lede {
    margin: calc(var(--space-2) - var(--space-4)) 0 var(--space-2);
    font-size: 0.9375rem;
    line-height: 1.5;
    color: var(--mut);
  }

  .waiting {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    font-weight: 600;
    color: var(--hd);
  }

  .waiting :global(svg) {
    animation: spin 1.1s linear infinite;
  }

  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .waiting :global(svg) {
      animation: none;
    }
  }

  .problem {
    margin: 0;
    font-size: 0.8125rem;
    color: var(--bad);
  }

  .primary,
  .secondary {
    border: none;
    font: inherit;
    font-weight: 700;
    font-size: 0.9375rem;
    padding: var(--space-3) var(--space-6);
    border-radius: var(--pill);
    cursor: pointer;
  }

  .primary {
    background: var(--acc);
    color: var(--b2f);
  }

  .primary:hover {
    background: var(--color-accent-600);
  }

  .secondary {
    background: var(--card2);
    color: var(--fg);
  }

  .secondary:hover {
    filter: brightness(0.96);
  }
</style>
