<script lang="ts">
  import { goto } from '$app/navigation';
  import { AlertDialog } from 'bits-ui';
  import { FileText, Folder, Globe, LockKeyhole } from 'lucide-svelte';
  import { commands, type FileKind, type SourceSummary } from '$lib/bindings';
  import Button from '$lib/components/Button.svelte';
  import SettingsTabs from '$lib/components/SettingsTabs.svelte';
  import { ago, shortDate } from '$lib/format';

  // S12 — Sources settings. A source syncs only when a scan runs, and a scan
  // runs only with the window open, so "synced" is the last scan's time.
  let sources = $state<SourceSummary[] | null>(null);
  let removing = $state<SourceSummary | null>(null);
  let busy = $state(false);
  let note = $state<string | null>(null);

  const kinds: Record<string, { badge: string; icon: typeof Globe; file: boolean }> = {
    imap: { badge: 'IMAP', icon: Globe, file: false },
    outlook: { badge: 'Outlook', icon: Globe, file: false },
    mbox: { badge: 'mbox', icon: FileText, file: true },
    maildir: { badge: 'Maildir', icon: Folder, file: true },
  };
  const kindOf = (s: SourceSummary) => kinds[s.kind] ?? kinds.imap;

  function load() {
    commands.listSources().then((r) => {
      if (r.status === 'ok') sources = r.data;
      else note = r.error;
    });
  }

  $effect(load);

  function synced(s: SourceSummary) {
    const file = kindOf(s).file;
    if (!s.lastSyncAt) return file ? 'not imported yet' : 'not synced yet';
    return file ? `imported ${shortDate(s.lastSyncAt)}` : `synced ${ago(s.lastSyncAt)}`;
  }

  async function choose(kind: FileKind) {
    note = null;
    const result = await commands.chooseFileSource(kind);
    if (result.status === 'ok') {
      if (result.data) await goto(`/scan?source=${result.data.id}`);
    } else if ('message' in result.error) note = result.error.message;
  }

  async function remove() {
    if (!removing) return;
    busy = true;
    const result = await commands.removeSource(removing.id);
    busy = false;
    removing = null;
    if (result.status === 'error') {
      note = result.error;
      return;
    }
    const left = await commands.listSources();
    if (left.status === 'ok' && left.data.length === 0) await goto('/welcome');
    else load();
  }
</script>

<SettingsTabs />

<div class="cards">
  {#each sources ?? [] as s (s.id)}
    {@const kind = kindOf(s)}
    <section class="card">
      <span class="icon"><kind.icon size={21} strokeWidth={2.75} /></span>
      <div class="grow">
        <div class="name">
          <h2>{s.label}</h2>
          <span class="badge">{kind.badge}</span>
        </div>
        <p>{s.messageCount.toLocaleString()} messages · {synced(s)}</p>
      </div>
      <Button variant="outline" size="small" onclick={() => goto(`/scan?source=${s.id}`)}>
        {kind.file ? 'Re-import' : 'Re-sync'}
      </Button>
      <button type="button" class="quiet" onclick={() => (removing = s)}>
        {kind.file ? 'Remove' : 'Disconnect'}
      </button>
    </section>
  {/each}

  <section class="add">
    <span>Add a source</span>
    <div class="buttons">
      <button type="button" onclick={() => choose('mbox')}>Import mbox</button>
      <button type="button" onclick={() => choose('maildir')}>Import Maildir</button>
      <button type="button" onclick={() => goto('/connect')}>Connect IMAP</button>
      <button type="button" onclick={() => goto('/outlook')}>Sign in to Outlook.com</button>
    </div>
  </section>

  {#if note}<p class="note" role="alert">{note}</p>{/if}

  <p class="foot">
    <LockKeyhole size={14} strokeWidth={2.75} />
    All sources are read on this computer. Sign-ins stay here, encrypted. A source syncs only when you
    scan it with the app open.
  </p>
</div>

<AlertDialog.Root open={removing !== null} onOpenChange={(open) => !open && (removing = null)}>
  <AlertDialog.Portal>
    <AlertDialog.Overlay class="remove-scrim" />
    <AlertDialog.Content class="remove-dialog">
      {#if removing}
        {@const file = kindOf(removing).file}
        <AlertDialog.Title class="remove-title">
          {file ? 'Remove' : 'Disconnect'}
          {removing.label}?
        </AlertDialog.Title>
        <AlertDialog.Description class="remove-text">
          Its {removing.messageCount.toLocaleString()} messages leave the local index{file
            ? ''
            : ', and its saved sign-in is deleted'}. The {file ? 'file' : 'mailbox'} itself is untouched,
          and the activity log keeps its entries.
        </AlertDialog.Description>
        <div class="remove-actions">
          <AlertDialog.Cancel class="remove-cancel">Keep it</AlertDialog.Cancel>
          <AlertDialog.Action class="remove-confirm" disabled={busy} onclick={remove}>
            {file ? 'Remove' : 'Disconnect'}
          </AlertDialog.Action>
        </div>
      {/if}
    </AlertDialog.Content>
  </AlertDialog.Portal>
</AlertDialog.Root>

<style>
  .cards {
    max-width: 50.75rem;
    margin-top: 1.625rem;
    display: flex;
    flex-direction: column;
    gap: 0.875rem;
  }

  .card {
    display: flex;
    align-items: center;
    gap: 1.125rem;
    background: var(--card);
    border: var(--stroke) solid var(--line);
    border-radius: var(--radius-lg);
    padding: 1.375rem 1.625rem;
    box-shadow: var(--shadow-sm);
  }

  .icon {
    flex: none;
    width: 2.875rem;
    height: 2.875rem;
    border-radius: 0.875rem;
    background: var(--sageSoft);
    color: var(--sageDeep);
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .grow {
    flex: 1;
    min-width: 0;
  }

  .name {
    display: flex;
    align-items: center;
    gap: 0.625rem;
  }

  h2 {
    margin: 0;
    font-size: 1rem;
    font-weight: 700;
    color: var(--hd);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .badge {
    flex: none;
    font-size: 0.656rem;
    font-weight: 800;
    color: var(--sageDeep);
    background: var(--sageSoft);
    padding: 0.125rem 0.5625rem;
    border-radius: var(--pill);
  }

  .card p {
    margin: 0.1875rem 0 0;
    font-size: 0.8125rem;
    color: var(--mut);
  }

  .quiet {
    cursor: pointer;
    background: transparent;
    border: var(--stroke-strong) solid transparent;
    border-radius: var(--pill);
    padding: 0.5625rem 1.125rem;
    font-family: var(--font-body);
    font-size: 0.8125rem;
    font-weight: 600;
    color: var(--mut);
  }

  .quiet:hover {
    color: var(--accDeep);
  }

  .add {
    display: flex;
    align-items: center;
    gap: 1rem;
    flex-wrap: wrap;
    border: var(--stroke-strong) dashed var(--line);
    border-radius: var(--radius-lg);
    padding: 1.25rem 1.625rem;
  }

  .add > span {
    font-size: 0.844rem;
    font-weight: 700;
    color: var(--mut);
  }

  .buttons {
    display: flex;
    flex-wrap: wrap;
    gap: 0.625rem;
  }

  .buttons button {
    cursor: pointer;
    background: var(--card);
    color: var(--fg);
    border: var(--stroke) solid var(--line);
    border-radius: var(--pill);
    padding: 0.625rem 1.25rem;
    font-family: var(--font-body);
    font-size: 0.8125rem;
    font-weight: 700;
  }

  .buttons button:hover {
    box-shadow: var(--shadow-sm);
  }

  .note {
    margin: 0;
    font-size: 0.8125rem;
    color: var(--accDeep);
  }

  .foot {
    display: flex;
    align-items: center;
    gap: 0.5625rem;
    margin: 0.375rem 0 0;
    font-size: 0.8125rem;
    color: var(--mut);
  }

  .foot :global(svg) {
    flex: none;
  }

  :global(.remove-scrim) {
    position: fixed;
    inset: 0;
    background: var(--scrim);
    z-index: 10;
  }

  :global(.remove-dialog) {
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

  :global(.remove-title) {
    margin: 0;
    font-family: var(--font-heading);
    font-weight: var(--font-heading-weight);
    font-size: 1.25rem;
    color: var(--hd);
    overflow-wrap: anywhere;
  }

  :global(.remove-text) {
    margin: 0.625rem 0 0;
    font-size: 0.875rem;
    line-height: 1.55;
    color: var(--mut);
  }

  :global(.remove-actions) {
    display: flex;
    justify-content: flex-end;
    gap: var(--space-2);
    margin-top: 1.375rem;
  }

  :global(.remove-cancel),
  :global(.remove-confirm) {
    cursor: pointer;
    font-family: var(--font-body);
    font-size: 0.84rem;
    font-weight: 700;
    padding: 0.625rem 1.125rem;
    border-radius: var(--pill);
  }

  :global(.remove-cancel) {
    background: transparent;
    color: var(--fg);
    border: var(--stroke-strong) solid var(--line);
  }

  :global(.remove-cancel:hover) {
    background: var(--card2);
  }

  :global(.remove-confirm) {
    border: none;
    background: var(--accDeep);
    color: var(--bg);
  }

  :global(.remove-confirm:disabled) {
    opacity: 0.45;
  }
</style>
