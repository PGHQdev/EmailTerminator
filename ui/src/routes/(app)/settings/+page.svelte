<script lang="ts">
  import { AlertDialog } from 'bits-ui';
  import { revealItemInDir } from '@tauri-apps/plugin-opener';
  import { KeyRound, LockKeyhole } from 'lucide-svelte';
  import { appearance } from '$lib/appearance.svelte';
  import { commands, type Appearance, type DataLocation } from '$lib/bindings';
  import Button from '$lib/components/Button.svelte';
  import { bytes } from '$lib/format';

  // S17 — General settings, M2's part: appearance, data location, encryption
  // and erase. Sweep behaviour arrives in M3, licence and cancel stats in M8,
  // updates in M9.
  let location = $state<DataLocation | null>(null);
  let note = $state<string | null>(null);
  let erasing = $state(false);
  let confirmOpen = $state(false);

  $effect(() => {
    commands.dataLocation().then((r) => {
      if (r.status === 'ok') location = r.data;
      else note = r.error;
    });
  });

  const choices: Array<{ value: Appearance; label: string }> = [
    { value: 'light', label: 'Light' },
    { value: 'dark', label: 'Dark' },
    { value: 'system', label: 'System' },
  ];

  async function move() {
    note = null;
    const result = await commands.moveDataLocation();
    // Success restarts the app; only a refusal or a closed picker returns.
    if (result.status === 'error') note = result.error;
    else if (result.data.kind === 'refused') note = result.data.message;
  }

  async function erase() {
    erasing = true;
    const result = await commands.eraseLocalData();
    if (result.status === 'error') note = result.error;
    erasing = false;
    confirmOpen = false;
  }
</script>

<header>
  <h1>Settings</h1>
  <div class="tabs" role="tablist">
    <span class="tab off" aria-disabled="true" title="Arrives with more sources">Sources</span>
    <span class="tab off" aria-disabled="true" title="Arrives with intelligence">Intelligence</span>
    <span class="tab on" role="tab" aria-selected="true">General</span>
  </div>
</header>

<div class="cards">
  <section class="card row">
    <div>
      <h2>Appearance</h2>
      <p>Follows the system by default</p>
    </div>
    <div class="segments" role="radiogroup" aria-label="Appearance">
      {#each choices as choice (choice.value)}
        <button
          type="button"
          role="radio"
          aria-checked={appearance.value === choice.value}
          class:on={appearance.value === choice.value}
          onclick={() => appearance.set(choice.value)}
        >
          {choice.label}
        </button>
      {/each}
    </div>
  </section>

  <section class="card">
    <div class="row">
      <div class="grow">
        <h2>Data location</h2>
        <p class="mono">
          {location ? location.path : '…'}{location?.bytes ? ` · ${bytes(location.bytes)}` : ''}
        </p>
      </div>
      <div class="buttons">
        <Button variant="outline" size="small" disabled={!location} onclick={() => location && revealItemInDir(location.path)}>
          Reveal
        </Button>
        <Button variant="outline" size="small" disabled={!location} onclick={move}>Move…</Button>
      </div>
    </div>
    {#if location}
      <!-- No mockup yet (DESIGN.md S17): the encryption backend, in plain words. -->
      <p class="lock">
        {#if location.keyPlace === 'keychain'}
          <LockKeyhole size={15} strokeWidth={2.75} />
          <span>
            Encrypted. The key is in your OS keychain, so a copy of this folder cannot be read
            without it.
          </span>
        {:else}
          <KeyRound size={15} strokeWidth={2.75} />
          <span>
            Encrypted, with the key in a file beside the data because this system has no keychain.
            A copy of the folder that includes the key file can be read.
          </span>
        {/if}
      </p>
    {/if}
  </section>

  <section class="card row">
    <div>
      <h2 class="danger">Erase everything</h2>
      <p>Delete the local index, saved app passwords and the key. Your mailbox itself is untouched.</p>
    </div>
    <Button variant="danger" size="small" onclick={() => (confirmOpen = true)}>Erase local data…</Button>
  </section>

  {#if note}<p class="note" role="alert">{note}</p>{/if}

  <p class="more">Something wrong? <a href="/report">Report an issue</a></p>
</div>

<AlertDialog.Root bind:open={confirmOpen}>
  <AlertDialog.Portal>
    <AlertDialog.Overlay class="erase-scrim" />
    <AlertDialog.Content class="erase-dialog">
      <AlertDialog.Title class="erase-title">Erase all local data?</AlertDialog.Title>
      <AlertDialog.Description class="erase-text">
        This deletes every scan result, the saved app passwords and the database key on this
        computer. Your mailbox is untouched, and a new scan reads it again. The app restarts.
      </AlertDialog.Description>
      <div class="erase-actions">
        <AlertDialog.Cancel class="erase-cancel">Keep my data</AlertDialog.Cancel>
        <AlertDialog.Action class="erase-confirm" disabled={erasing} onclick={erase}>
          Erase and restart
        </AlertDialog.Action>
      </div>
    </AlertDialog.Content>
  </AlertDialog.Portal>
</AlertDialog.Root>

<style>
  header {
    display: flex;
    align-items: baseline;
    gap: 1rem;
    flex-wrap: wrap;
  }

  h1 {
    margin: 0;
    font-family: var(--font-heading);
    font-weight: var(--font-heading-weight);
    font-size: 1.875rem;
    color: var(--hd);
  }

  .tabs,
  .segments {
    display: flex;
    gap: 0.25rem;
    background: var(--card2);
    border-radius: var(--pill);
    padding: 0.25rem;
  }

  .tab,
  .segments button {
    padding: 0.4375rem 1.125rem;
    border-radius: var(--pill);
    font-size: 0.8125rem;
    font-weight: 600;
    color: var(--mut);
  }

  .tab.off {
    opacity: 0.45;
  }

  .tab.on,
  .segments button.on {
    background: var(--card);
    color: var(--hd);
    font-weight: 700;
    box-shadow: var(--shadow-sm);
  }

  .segments button {
    border: none;
    background: transparent;
    cursor: pointer;
    font-family: var(--font-body);
    font-size: 0.78rem;
    padding: 0.4375rem 1rem;
  }

  .segments button:hover {
    color: var(--fg);
  }

  .cards {
    max-width: 47.5rem;
    margin-top: 1.625rem;
    display: flex;
    flex-direction: column;
    gap: 0.875rem;
  }

  .card {
    background: var(--card);
    border: var(--stroke) solid var(--line);
    border-radius: var(--radius-lg);
    padding: 1.25rem 1.625rem;
    box-shadow: var(--shadow-sm);
  }

  .row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 1rem;
  }

  .grow {
    min-width: 0;
  }

  .buttons {
    display: flex;
    gap: var(--space-2);
    flex: none;
  }

  h2 {
    margin: 0;
    font-size: 0.94rem;
    font-weight: 700;
    color: var(--hd);
  }

  h2.danger {
    color: var(--accDeep);
  }

  .card p {
    margin: 0.125rem 0 0;
    font-size: 0.8125rem;
    color: var(--mut);
  }

  .mono {
    font-family: var(--font-mono);
    overflow-wrap: anywhere;
  }

  .card .lock {
    display: flex;
    gap: 0.5rem;
    margin-top: 0.875rem;
    padding-top: 0.875rem;
    border-top: var(--stroke) solid var(--line);
    line-height: 1.5;
  }

  .lock :global(svg) {
    flex: none;
    margin-top: 0.125rem;
    color: var(--sageDeep);
  }

  .note {
    margin: 0;
    font-size: 0.8125rem;
    color: var(--accDeep);
  }

  .more {
    margin: 0.5rem 0 0;
    font-size: 0.8125rem;
    color: var(--mut);
  }

  .more a {
    font-weight: 700;
    color: var(--accDeep);
    text-decoration: none;
  }

  :global(.erase-scrim) {
    position: fixed;
    inset: 0;
    background: var(--scrim);
    z-index: 10;
  }

  :global(.erase-dialog) {
    position: fixed;
    left: 50%;
    top: 50%;
    transform: translate(-50%, -50%);
    width: min(27rem, calc(100vw - 2rem));
    box-sizing: border-box;
    background: var(--card);
    border-radius: var(--radius-lg);
    box-shadow: var(--shadow-lg);
    padding: 1.625rem;
    z-index: 11;
    font-family: var(--font-body);
  }

  :global(.erase-title) {
    margin: 0;
    font-family: var(--font-heading);
    font-weight: var(--font-heading-weight);
    font-size: 1.25rem;
    color: var(--hd);
  }

  :global(.erase-text) {
    margin: 0.625rem 0 0;
    font-size: 0.875rem;
    line-height: 1.55;
    color: var(--mut);
  }

  :global(.erase-actions) {
    display: flex;
    justify-content: flex-end;
    gap: var(--space-2);
    margin-top: 1.375rem;
  }

  :global(.erase-cancel),
  :global(.erase-confirm) {
    cursor: pointer;
    font-family: var(--font-body);
    font-size: 0.84rem;
    font-weight: 700;
    padding: 0.625rem 1.125rem;
    border-radius: var(--pill);
  }

  :global(.erase-cancel) {
    background: transparent;
    color: var(--fg);
    border: var(--stroke-strong) solid var(--line);
  }

  :global(.erase-cancel:hover) {
    background: var(--card2);
  }

  :global(.erase-confirm) {
    border: none;
    background: var(--accDeep);
    color: var(--bg);
  }

  :global(.erase-confirm:hover) {
    filter: brightness(1.1);
  }

  :global(.erase-confirm:disabled) {
    opacity: 0.45;
  }
</style>
