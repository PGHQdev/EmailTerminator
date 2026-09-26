<script lang="ts">
  import { AlertDialog } from 'bits-ui';
  import { TriangleAlert } from 'lucide-svelte';
  import { sweep } from '$lib/sweep.svelte';

  // S10 — a critical service asks before anything touches it. The user types
  // the phrase, so a confirmation is never one careless click.
  let typed = $state('');
  let asking = $derived(sweep.asking);
  let phrase = $derived(asking ? `${asking.action} ${asking.name.toLowerCase()}` : '');
  let matches = $derived(typed.trim().toLowerCase() === phrase);

  $effect(() => {
    if (asking) typed = '';
  });

  function answer(yes: boolean) {
    asking?.answer(yes && matches);
  }
</script>

<AlertDialog.Root open={asking !== null} onOpenChange={(open) => !open && answer(false)}>
  <AlertDialog.Portal>
    <AlertDialog.Overlay class="critical-scrim" />
    <AlertDialog.Content class="critical-dialog">
      <div class="band">
        <span class="badge"><TriangleAlert size={19} strokeWidth={2.75} /></span>
        <AlertDialog.Title class="critical-title">{asking?.name} looks critical</AlertDialog.Title>
      </div>
      <div class="body">
        <AlertDialog.Description class="critical-text">
          {#if asking?.action === 'cancel'}
            Cancelling could break things you rely on at {asking?.name}, such as security alerts,
            sign-in codes, billing notices or the service itself.
          {:else}
            Unsubscribing could stop mail you rely on from {asking?.name}, such as security alerts,
            sign-in codes or billing notices. Receipts keep arriving either way.
          {/if}
        </AlertDialog.Description>
        <label class="prompt" for="critical-phrase">
          Type <strong>{phrase}</strong> to confirm you understand.
        </label>
        <input
          id="critical-phrase"
          bind:value={typed}
          autocomplete="off"
          spellcheck="false"
          onkeydown={(e) => e.key === 'Enter' && matches && answer(true)}
        />
        <div class="buttons">
          <AlertDialog.Cancel class="critical-keep">Keep it</AlertDialog.Cancel>
          <AlertDialog.Action class="critical-go" disabled={!matches} onclick={() => answer(true)}>
            {asking?.action === 'cancel' ? 'Cancel anyway' : 'Unsubscribe anyway'}
          </AlertDialog.Action>
        </div>
      </div>
    </AlertDialog.Content>
  </AlertDialog.Portal>
</AlertDialog.Root>

<style>
  :global(.critical-scrim) {
    position: fixed;
    inset: 0;
    background: var(--scrim);
    z-index: 20;
  }

  :global(.critical-dialog) {
    position: fixed;
    left: 50%;
    top: 50%;
    transform: translate(-50%, -50%);
    width: min(31.25rem, calc(100vw - 2rem));
    background: var(--card);
    border-radius: var(--radius-lg);
    box-shadow: var(--shadow-lg);
    overflow: hidden;
    z-index: 21;
    font-family: var(--font-body);
  }

  .band {
    display: flex;
    align-items: center;
    gap: 0.75rem;
    padding: 1.125rem 1.875rem;
    background: var(--accSoft);
  }

  .badge {
    width: 2.375rem;
    height: 2.375rem;
    flex: none;
    border-radius: var(--pill);
    background: var(--acc);
    color: var(--onAcc);
    display: flex;
    align-items: center;
    justify-content: center;
  }

  :global(.critical-title) {
    margin: 0;
    font-family: var(--font-heading);
    font-weight: var(--font-heading-weight);
    font-size: 1.25rem;
    color: var(--accDeep);
  }

  .body {
    padding: 1.5rem 1.875rem 1.875rem;
  }

  :global(.critical-text) {
    margin: 0;
    font-size: 0.906rem;
    line-height: 1.6;
    color: var(--fg);
  }

  .prompt {
    display: block;
    margin-top: 1.25rem;
    font-size: 0.8125rem;
    color: var(--mut);
  }

  .prompt strong {
    font-family: var(--font-mono);
    color: var(--fg);
  }

  input {
    box-sizing: border-box;
    width: 100%;
    margin-top: 0.625rem;
    padding: 0.75rem 1.25rem;
    border: var(--stroke-strong) solid var(--line);
    border-radius: var(--pill);
    background: var(--card2);
    font-family: var(--font-mono);
    font-size: 0.875rem;
    color: var(--fg);
  }

  input:focus {
    outline: none;
    border-color: var(--acc);
  }

  .buttons {
    display: flex;
    gap: 0.625rem;
    margin-top: 1.25rem;
  }

  .buttons :global(button) {
    flex: 1;
    cursor: pointer;
    font-family: var(--font-body);
    font-size: 0.875rem;
    font-weight: 700;
    padding: 0.75rem 0;
    border-radius: var(--pill);
  }

  :global(.critical-keep) {
    background: transparent;
    color: var(--fg);
    border: var(--stroke-strong) solid var(--line);
  }

  :global(.critical-keep:hover) {
    background: var(--card2);
  }

  :global(.critical-go) {
    border: none;
    background: var(--acc);
    color: var(--onAcc);
  }

  :global(.critical-go:disabled) {
    opacity: 0.45;
    cursor: default;
  }
</style>
