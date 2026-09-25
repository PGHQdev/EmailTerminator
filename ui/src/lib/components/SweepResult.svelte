<script lang="ts">
  import { page } from '$app/state';
  import { openUrl } from '@tauri-apps/plugin-opener';
  import { Dialog } from 'bits-ui';
  import { Check, X } from 'lucide-svelte';
  import { evidence } from '$lib/evidence.svelte';
  import { count } from '$lib/format';
  import { sweep } from '$lib/sweep.svelte';

  // S07 — the result of a sweep that ran without S11. S11 shows its own
  // results and problems, so this stays closed there.
  let rows = $derived(sweep.result ?? []);
  let open = $derived(
    page.url.pathname !== '/sweep' && (sweep.result !== null || sweep.problem !== null),
  );
  let done = $derived(rows.filter((r) => r.result?.outcome === 'succeeded'));
  let removed = $derived(done.reduce((t, r) => t + r.emailsPerYear, 0));

  function names(list: string[]): string {
    if (list.length <= 1) return list[0] ?? '';
    return `${list.slice(0, -1).join(', ')} and ${list.at(-1)}`;
  }

  let doneButton = $state<HTMLButtonElement | null>(null);

  function close() {
    sweep.closeResult();
  }
</script>

<Dialog.Root {open} onOpenChange={(o) => !o && close()}>
  <Dialog.Portal>
    <Dialog.Overlay class="result-scrim" />
    <Dialog.Content
      class="result-dialog"
      onOpenAutoFocus={(e) => {
        e.preventDefault();
        doneButton?.focus();
      }}
    >
      {#if sweep.problem && rows.length === 0}
        <span class="badge bad"><X size={34} strokeWidth={2.75} /></span>
        <Dialog.Title class="result-title">The sweep did not start</Dialog.Title>
        <Dialog.Description class="result-text">{sweep.problem}</Dialog.Description>
      {:else}
        <span class="badge" class:bad={done.length === 0}>
          {#if done.length > 0}<Check size={34} strokeWidth={2.75} />{:else}<X size={34} strokeWidth={2.75} />{/if}
        </span>
        <Dialog.Title class="result-title">{done.length} of {rows.length} unsubscribed</Dialog.Title>
        <Dialog.Description class="result-text">
          {#if done.length > 0}
            {names(done.map((r) => r.name))} honored one-click unsubscribe. That's
            <strong>{count(removed)} fewer {removed === 1 ? 'email' : 'emails'} a year</strong>.
          {:else}
            Nothing was unsubscribed. Each row says why.
          {/if}
        </Dialog.Description>
        <ul>
          {#each rows as row (row.target.kind + row.target.id)}
            {@const r = row.result}
            <li>
              <span>{row.name}</span>
              {#if !r}
                <span class="mut">not run</span>
              {:else if r.outcome === 'succeeded'}
                <span class="ok">
                  <Check size={12} strokeWidth={3} />done ·
                  <button type="button" onclick={() => evidence.open(r.actionIds[0])}>evidence</button>
                </span>
              {:else if r.outcome === 'needsYou' && r.finishAt}
                {@const at = r.finishAt}
                <span class="acc">
                  needs you ·
                  <button type="button" onclick={() => openUrl(at)}>
                    {at.toLowerCase().startsWith('mailto:') ? 'write the email' : 'open page'}
                  </button>
                </span>
              {:else}
                <span class="bad" title={r.detail}>
                  failed ·
                  {#if r.actionIds.length > 0}
                    <button type="button" onclick={() => evidence.open(r.actionIds[0])}>why</button>
                  {:else}
                    {r.detail}
                  {/if}
                </span>
              {/if}
            </li>
          {/each}
        </ul>
      {/if}
      <Dialog.Close class="result-done" bind:ref={doneButton}>Done</Dialog.Close>
      {#if done.length > 0}
        <p class="foot">Senders can take a few days to stop.</p>
      {/if}
    </Dialog.Content>
  </Dialog.Portal>
</Dialog.Root>

<style>
  :global(.result-scrim) {
    position: fixed;
    inset: 0;
    background: var(--scrim);
    z-index: 10;
  }

  :global(.result-dialog) {
    position: fixed;
    left: 50%;
    top: 50%;
    transform: translate(-50%, -50%);
    width: min(28.75rem, calc(100vw - 2rem));
    box-sizing: border-box;
    background: var(--card);
    border-radius: var(--radius-lg);
    box-shadow: var(--shadow-lg);
    padding: 2.125rem 2.25rem;
    text-align: center;
    z-index: 11;
    font-family: var(--font-body);
    color: var(--fg);
  }

  .badge {
    width: 4.625rem;
    height: 4.625rem;
    margin: 0 auto;
    border-radius: var(--pill);
    background: var(--sageSoft);
    color: var(--sageDeep);
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .badge.bad {
    background: var(--accSoft);
    color: var(--accDeep);
  }

  :global(.result-title) {
    margin: 1.125rem 0 0;
    font-family: var(--font-heading);
    font-weight: var(--font-heading-weight);
    font-size: 1.5rem;
    color: var(--hd);
  }

  :global(.result-text) {
    margin: 0.5rem 0 0;
    font-size: 0.906rem;
    line-height: 1.55;
    color: var(--mut);
  }

  :global(.result-text) strong {
    color: var(--fg);
  }

  ul {
    list-style: none;
    margin: 1.125rem 0 0;
    padding: 0.75rem 1rem;
    display: flex;
    flex-direction: column;
    gap: 0.4375rem;
    text-align: left;
    font-size: 0.8125rem;
    color: var(--mut);
    background: var(--card2);
    border-radius: 0.875rem;
  }

  li {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 0.75rem;
  }

  li > span:last-child {
    display: flex;
    align-items: center;
    gap: 0.3125rem;
    font-weight: 700;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .ok {
    color: var(--ok);
  }

  .acc {
    color: var(--accDeep);
  }

  .bad {
    color: var(--bad);
  }

  li button {
    padding: 0;
    border: none;
    background: none;
    font: inherit;
    color: var(--accDeep);
    cursor: pointer;
  }

  li button:hover {
    opacity: 0.8;
  }

  :global(.result-done) {
    width: 100%;
    margin-top: 1.25rem;
    border: none;
    cursor: pointer;
    background: var(--sageDeep);
    color: var(--onSage);
    font-family: var(--font-body);
    font-size: 0.906rem;
    font-weight: 700;
    padding: 0.8125rem 0;
    border-radius: var(--pill);
  }

  :global(.result-done:hover) {
    filter: brightness(1.15);
  }

  .foot {
    margin: 0.75rem 0 0;
    font-size: 0.75rem;
    color: var(--mut);
  }
</style>
