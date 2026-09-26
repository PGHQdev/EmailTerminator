<script lang="ts">
  import { goto } from '$app/navigation';
  import { openUrl } from '@tauri-apps/plugin-opener';
  import { ArrowLeft } from 'lucide-svelte';
  import { commands, type ConnectError, type ImapProvider, type ProviderInfo } from '$lib/bindings';
  import Brand from '$lib/components/Brand.svelte';
  import SignInFailed from '$lib/components/SignInFailed.svelte';

  // The IMAP connect form: no mockup; it follows S01's card and S16's error.
  let providers = $state<ProviderInfo[]>([]);
  let provider = $state<ImapProvider>('gmail');
  let email = $state('');
  let password = $state('');
  let host = $state('');
  let port = $state(993);
  let busy = $state(false);
  let error = $state<ConnectError | null>(null);
  let passwordField: HTMLInputElement | undefined = $state();

  let chosen = $derived(providers.find((p) => p.provider === provider));

  $effect(() => {
    commands.imapPresets().then((list) => (providers = list));
  });

  async function connect(event: SubmitEvent) {
    event.preventDefault();
    busy = true;
    error = null;
    const result = await commands.addImapSource({
      provider,
      email,
      password,
      host: provider === 'other' ? host : null,
      port: provider === 'other' ? port : null,
    });
    busy = false;
    if (result.status === 'ok') {
      password = '';
      await goto(`/scan?source=${result.data.id}`);
    } else {
      error = result.error;
    }
  }

  function retry() {
    error = null;
    password = '';
    passwordField?.focus();
  }
</script>

<main>
  <header>
    <button type="button" class="back" onclick={() => goto('/')}>
      <ArrowLeft size={16} strokeWidth={2.75} /> Back
    </button>
    <Brand size="small" />
  </header>

  <section>
    <h1>Connect via IMAP</h1>
    <p class="lede">
      Use an app password from your provider. Your regular password will not work, and the app
      password stays in this computer's keychain.
    </p>

    {#if error?.kind === 'signInRefused'}
      <SignInFailed host={error.host} serverSays={error.serverSays} guide={error.guide} onRetry={retry} />
    {:else}
      <form onsubmit={connect}>
        <fieldset class="providers">
          <legend>Provider</legend>
          {#each providers as p (p.provider)}
            <label class:on={provider === p.provider}>
              <input type="radio" name="provider" value={p.provider} bind:group={provider} />
              {p.label}
            </label>
          {/each}
        </fieldset>

        {#if provider === 'other'}
          <div class="row">
            <label class="field grow">
              IMAP server
              <input bind:value={host} placeholder="imap.example.com" autocomplete="off" required />
            </label>
            <label class="field port">
              Port
              <input type="number" bind:value={port} min="1" max="65535" required />
            </label>
          </div>
        {/if}

        <label class="field">
          Email address
          <input type="email" bind:value={email} autocomplete="username" required />
        </label>
        <label class="field">
          App password
          <input
            type="password"
            bind:value={password}
            bind:this={passwordField}
            autocomplete="current-password"
            required
          />
        </label>

        {#if chosen?.guide}
          <button type="button" class="guide" onclick={() => chosen?.guide && openUrl(chosen.guide)}>
            How to create an app password for {chosen.label}
          </button>
        {/if}
        {#if provider === 'gmail'}
          <p class="hint">
            No App passwords page? Google shows it only when your second step is an authenticator
            app or a phone number. A passkey or security key alone is not enough.
          </p>
        {/if}

        {#if error}
          <p class="problem" role="alert">{error.message}</p>
        {/if}

        <button type="submit" class="primary" disabled={busy}>
          {busy ? 'Checking the password…' : 'Connect'}
        </button>
      </form>
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
  }

  h1 {
    margin: 0;
    font-family: var(--font-heading);
    font-weight: var(--font-heading-weight);
    font-size: 2rem;
    color: var(--hd);
  }

  .lede {
    margin: var(--space-2) 0 var(--space-6);
    font-size: 0.9375rem;
    line-height: 1.5;
    color: var(--mut);
  }

  form {
    display: flex;
    flex-direction: column;
    gap: var(--space-4);
  }

  .providers {
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-2);
    border: none;
    padding: 0;
    margin: 0;
  }

  legend {
    font-size: 0.8125rem;
    font-weight: 600;
    color: var(--mut);
    margin-bottom: var(--space-2);
  }

  .providers label {
    padding: var(--space-2) var(--space-3);
    border: var(--stroke-strong) solid var(--line);
    border-radius: var(--pill);
    font-size: 0.8125rem;
    font-weight: 600;
    cursor: pointer;
  }

  .providers label:hover {
    border-color: var(--mut);
  }

  .providers label.on {
    background: var(--sage);
    border-color: var(--sage);
    color: var(--onSage);
  }

  .providers label:has(input:focus-visible) {
    outline: var(--stroke-strong) solid var(--color-accent);
    outline-offset: var(--stroke-strong);
  }

  .providers input {
    position: absolute;
    opacity: 0;
    width: 0;
    height: 0;
  }

  .row {
    display: flex;
    gap: var(--space-3);
  }

  .grow {
    flex: 1;
  }

  .port {
    width: 6rem;
  }

  .field {
    display: flex;
    flex-direction: column;
    gap: var(--space-1);
    font-size: 0.8125rem;
    font-weight: 600;
    color: var(--mut);
  }

  .field input {
    font: inherit;
    font-weight: 400;
    font-size: 0.9375rem;
    color: var(--fg);
    background: var(--card);
    border: var(--stroke) solid var(--line);
    border-radius: var(--radius-md);
    padding: var(--space-2) var(--space-3);
  }

  .field input:hover {
    border-color: var(--mut);
  }

  .field input:focus-visible {
    border-color: var(--acc);
  }

  .guide {
    align-self: flex-start;
    border: none;
    background: none;
    padding: 0;
    font: inherit;
    font-size: 0.8125rem;
    font-weight: 700;
    color: var(--accDeep);
    cursor: pointer;
  }

  .guide:hover {
    text-decoration: underline;
  }

  .hint {
    margin: 0;
    font-size: 0.8125rem;
    line-height: 1.5;
    color: var(--mut);
  }

  .problem {
    margin: 0;
    font-size: 0.8125rem;
    color: var(--bad);
  }

  .primary {
    align-self: flex-start;
    border: none;
    background: var(--acc);
    color: var(--b2f);
    font: inherit;
    font-weight: 700;
    font-size: 0.9375rem;
    padding: var(--space-3) var(--space-6);
    border-radius: var(--pill);
    cursor: pointer;
  }

  .primary:hover {
    background: var(--color-accent-600);
  }

  .primary:active {
    background: var(--color-accent-700);
  }

  .primary:disabled {
    opacity: 0.45;
    cursor: default;
  }
</style>
