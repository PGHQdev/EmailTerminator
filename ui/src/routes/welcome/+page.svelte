<script lang="ts">
  import { goto } from '$app/navigation';
  import { getCurrentWebview } from '@tauri-apps/api/webview';
  import { ArrowRight, FileArchive, KeyRound, LockKeyhole, Server } from 'lucide-svelte';
  import { commands, type ConnectError, type FileKind, type SourceSummary } from '$lib/bindings';
  import Brand from '$lib/components/Brand.svelte';

  let sources = $state<SourceSummary[]>([]);
  let dropping = $state(false);
  let problem = $state<string | null>(null);

  $effect(() => {
    commands.listSources().then((result) => {
      if (result.status === 'ok') sources = result.data;
    });
  });

  // "Drop an export": a file is an mbox, a folder a Maildir.
  $effect(() => {
    let stop: (() => void) | undefined;
    getCurrentWebview()
      .onDragDropEvent(async (event) => {
        const drag = event.payload;
        dropping = drag.type === 'enter' || drag.type === 'over';
        if (drag.type !== 'drop' || drag.paths.length === 0) return;
        opened(await commands.addDroppedSource(drag.paths[0]));
      })
      .then((unlisten) => (stop = unlisten))
      .catch(() => {});
    return () => stop?.();
  });

  async function choose(kind: FileKind) {
    const result = await commands.chooseFileSource(kind);
    // A closed picker is `null`: nothing to do.
    if (result.status === 'error' || result.data) opened(result);
  }

  function opened(
    result:
      | { status: 'ok'; data: SourceSummary | null }
      | { status: 'error'; error: ConnectError },
  ) {
    if (result.status === 'ok') goto(`/scan?source=${result.data?.id}`);
    else problem = 'message' in result.error ? result.error.message : null;
  }
</script>

<!-- S01 — Welcome / source picker -->
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
    <div class="card file" class:dropping>
      <button type="button" class="cover" aria-label="Import an mbox file" onclick={() => choose('mbox')}
      ></button>
      <div class="icon"><FileArchive size={22} strokeWidth={2.75} /></div>
      <h2>Import an mbox file</h2>
      <p>Drop an export from any mail app. Fastest, fully offline.</p>
      <span class="cta">Choose file <ArrowRight size={15} strokeWidth={2.75} /></span>
      <button type="button" class="maildir" onclick={() => choose('maildir')}>
        or a Maildir folder
      </button>
    </div>

    <button type="button" class="card" onclick={() => goto('/connect')}>
      <div class="icon"><Server size={22} strokeWidth={2.75} /></div>
      <h2>Connect via IMAP</h2>
      <p>Any mailbox, with an app password. Stays in sync.</p>
      <span class="cta">Connect <ArrowRight size={15} strokeWidth={2.75} /></span>
    </button>

    <button type="button" class="card" onclick={() => goto('/outlook')}>
      <div class="icon"><KeyRound size={22} strokeWidth={2.75} /></div>
      <h2>Sign in to Outlook.com</h2>
      <p>Outlook, Hotmail and Live. Sign in with Microsoft. Stays in sync.</p>
      <span class="cta">Sign in <ArrowRight size={15} strokeWidth={2.75} /></span>
    </button>
  </div>

  {#if problem}
    <p class="problem" role="alert">{problem}</p>
  {/if}

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
    display: flex;
    flex-direction: column;
    align-items: flex-start;
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

  /* The mbox card holds two actions: the whole card picks a file, and its
     Maildir link sits above that cover. */
  .card.file:hover {
    box-shadow: var(--shadow-md);
    transform: translateY(-0.125rem);
  }

  .card.file.dropping {
    border: var(--stroke-strong) dashed var(--sage);
    box-shadow: var(--shadow-md);
  }

  .cover {
    position: absolute;
    inset: 0;
    border: none;
    background: none;
    border-radius: inherit;
    cursor: pointer;
  }

  .maildir {
    position: relative;
    margin-top: var(--space-2);
    padding: 0;
    border: none;
    background: none;
    font: inherit;
    font-size: 0.8125rem;
    font-weight: 600;
    color: var(--mut);
    cursor: pointer;
  }

  .maildir:hover {
    color: var(--fg);
    text-decoration: underline;
  }

  .problem {
    margin: var(--space-6) 0 0;
    font-size: 0.875rem;
    color: var(--bad);
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
