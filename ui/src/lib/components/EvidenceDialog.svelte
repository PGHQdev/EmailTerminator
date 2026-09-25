<script lang="ts">
  import { Dialog } from 'bits-ui';
  import { commands, type Original } from '$lib/bindings';
  import { actionLabel, outcomeLabel, tone } from '$lib/activity';
  import { evidence } from '$lib/evidence.svelte';
  import { clock, longDate } from '$lib/format';

  // An evidence link (S07, S11, S14), no mockup yet: what the app did, what
  // it sent, and the email it acted on, which can be read again from the
  // mailbox (PLAN.md Part 4).
  let entry = $derived(evidence.entry);
  let original = $state<Original | null>(null);
  let loading = $state(false);

  $effect(() => {
    if (entry) original = null;
  });

  async function read() {
    if (!entry) return;
    loading = true;
    const result = await commands.evidenceOriginal(entry.id);
    loading = false;
    original =
      result.status === 'ok' ? result.data : { kind: 'unreachable', message: result.error };
  }
</script>

<Dialog.Root open={entry !== null} onOpenChange={(o) => !o && evidence.close()}>
  <Dialog.Portal>
    <Dialog.Overlay class="evidence-scrim" />
    <Dialog.Content class="evidence-dialog">
      {#if entry}
        {@const e = entry.evidence}
        <Dialog.Title class="evidence-title">{actionLabel(entry)} · {entry.target}</Dialog.Title>
        <dl>
          <dt>When</dt>
          <dd>{longDate(entry.at)}, {clock(entry.at)}</dd>
          <dt>Outcome</dt>
          <dd class={tone(entry)}>{outcomeLabel(entry)}</dd>
          {#if entry.kind === 'unsubscribe'}
            <dt>Sent</dt>
            <dd class="mono">{entry.request ?? 'Nothing. The app sent no request.'}</dd>
          {/if}
          {#if e}
            <dt>Email</dt>
            <dd>
              <strong>{e.subject ?? '(no subject)'}</strong>
              <small>{e.from ?? 'unknown sender'} · {longDate(e.date)}</small>
            </dd>
          {/if}
        </dl>

        {#if e?.messageId !== null && e?.messageId !== undefined}
          {#if !original}
            <button type="button" class="read" disabled={loading} onclick={read}>
              {loading ? 'Reading the mailbox…' : 'Show the original email'}
            </button>
          {:else if original.kind === 'found'}
            <div class="original">
              <table>
                <tbody>
                  {#each original.preview.headers as field (field.name)}
                    <tr><th>{field.name}</th><td>{field.value}</td></tr>
                  {/each}
                </tbody>
              </table>
              <pre>{original.preview.text}</pre>
            </div>
          {:else if original.kind === 'gone'}
            <p class="note">
              The original is no longer in the mailbox. The subject, sender and date above are what
              the app kept.
            </p>
          {:else}
            <p class="note bad">The mailbox did not answer: {original.message}</p>
          {/if}
        {:else if e}
          <p class="note">
            The original is no longer in the mailbox. The subject, sender and date above are what the
            app kept.
          </p>
        {/if}
        {#if evidence.problem}<p class="note bad">{evidence.problem}</p>{/if}
        <Dialog.Close class="evidence-close">Close</Dialog.Close>
      {/if}
    </Dialog.Content>
  </Dialog.Portal>
</Dialog.Root>

<style>
  :global(.evidence-scrim) {
    position: fixed;
    inset: 0;
    background: var(--scrim);
    z-index: 12;
  }

  :global(.evidence-dialog) {
    position: fixed;
    left: 50%;
    top: 50%;
    transform: translate(-50%, -50%);
    width: min(38.75rem, calc(100vw - 2rem));
    max-height: calc(100vh - 4rem);
    overflow-y: auto;
    box-sizing: border-box;
    background: var(--card);
    border-radius: var(--radius-lg);
    box-shadow: var(--shadow-lg);
    padding: 1.75rem 2rem;
    z-index: 13;
    font-family: var(--font-body);
    color: var(--fg);
  }

  :global(.evidence-title) {
    margin: 0;
    font-family: var(--font-heading);
    font-weight: var(--font-heading-weight);
    font-size: 1.25rem;
    color: var(--hd);
  }

  dl {
    display: grid;
    grid-template-columns: 5.5rem 1fr;
    gap: 0.625rem 1rem;
    margin: 1.25rem 0 0;
    font-size: 0.875rem;
  }

  dt {
    font-size: 0.72rem;
    font-weight: 700;
    letter-spacing: 0.07em;
    text-transform: uppercase;
    color: var(--mut);
    padding-top: 0.125rem;
  }

  dd {
    margin: 0;
    min-width: 0;
    overflow-wrap: anywhere;
  }

  dd small {
    display: block;
    margin-top: 0.125rem;
    color: var(--mut);
  }

  .mono {
    font-family: var(--font-mono);
    font-size: 0.8125rem;
  }

  .ok {
    color: var(--ok);
    font-weight: 700;
  }

  .bad {
    color: var(--bad);
  }

  dd.bad {
    font-weight: 700;
  }

  .mut {
    color: var(--mut);
  }

  .read {
    margin-top: 1.25rem;
    background: transparent;
    color: var(--fg);
    font-family: var(--font-body);
    font-size: 0.8125rem;
    font-weight: 700;
    padding: 0.5625rem 1.125rem;
    border-radius: var(--pill);
    border: var(--stroke-strong) solid var(--line);
    cursor: pointer;
  }

  .read:hover {
    background: var(--card2);
  }

  .read:disabled {
    opacity: 0.45;
    cursor: default;
  }

  .original {
    margin-top: 1.25rem;
    background: var(--card2);
    border-radius: 0.875rem;
    padding: 0.875rem 1rem;
  }

  table {
    border-collapse: collapse;
    font-family: var(--font-mono);
    font-size: 0.72rem;
  }

  th {
    text-align: left;
    vertical-align: top;
    padding: 0.125rem 0.75rem 0.125rem 0;
    color: var(--mut);
    font-weight: 700;
    white-space: nowrap;
  }

  td {
    padding: 0.125rem 0;
    overflow-wrap: anywhere;
  }

  pre {
    margin: 0.875rem 0 0;
    max-height: 16rem;
    overflow-y: auto;
    white-space: pre-wrap;
    font-family: var(--font-body);
    font-size: 0.8125rem;
    line-height: 1.5;
  }

  .note {
    margin: 1.25rem 0 0;
    font-size: 0.8125rem;
    color: var(--mut);
  }

  :global(.evidence-close) {
    display: block;
    margin: 1.5rem 0 0 auto;
    background: var(--fg);
    color: var(--bg);
    border: none;
    cursor: pointer;
    font-family: var(--font-body);
    font-size: 0.875rem;
    font-weight: 700;
    padding: 0.625rem 1.5rem;
    border-radius: var(--pill);
  }
</style>
