<script lang="ts">
  import { goto } from '$app/navigation';
  import { page } from '$app/state';
  import { Channel } from '@tauri-apps/api/core';
  import { commands, type ScanError, type ScanEvent, type ScanSummary, type SourceSummary } from '$lib/bindings';
  import Brand from '$lib/components/Brand.svelte';
  import SignInFailed from '$lib/components/SignInFailed.svelte';

  type Phase = 'fetching' | 'settling' | 'done' | 'failed';

  const sourceId = Number(page.url.searchParams.get('source'));
  let source = $state<SourceSummary | null>(null);
  let phase = $state<Phase>('fetching');
  let fetched = $state(0);
  let total = $state<number | null>(null);
  let senders = $state(0);
  let subscriptions = $state(0);
  let newsletters = $state(0);
  let latestFind = $state<string | null>(null);
  let summary = $state<ScanSummary | null>(null);
  let error = $state<ScanError | null>(null);

  let percent = $derived(
    phase === 'done' ? 100 : total ? Math.min(99, Math.floor((fetched / total) * 100)) : 0,
  );
  const n = (value: number) => value.toLocaleString();

  // The ring: circumference of r = 104 in a 230 box.
  const ring = 2 * Math.PI * 104;

  async function start() {
    phase = 'fetching';
    error = null;
    const events = new Channel<ScanEvent>();
    events.onmessage = (event) => {
      if (event.kind === 'progress') {
        ({ fetched, senders, subscriptions, newsletters } = event);
        total = event.total;
        if (event.latestFind) latestFind = event.latestFind;
      } else {
        phase = 'settling';
      }
    };
    const result = await commands.startScan(sourceId, events);
    if (result.status === 'ok') {
      summary = result.data;
      ({ senders, subscriptions, newsletters } = result.data);
      phase = 'done';
    } else {
      error = result.error;
      phase = 'failed';
    }
  }

  $effect(() => {
    commands.listSources().then((result) => {
      if (result.status === 'ok') source = result.data.find((s) => s.id === sourceId) ?? null;
    });
    start();
  });
</script>

<!-- S02 — Scan progress -->
<main>
  <header><Brand size="small" /></header>

  <section>
    <div class="ring">
      <svg viewBox="0 0 230 230" aria-hidden="true">
        <circle cx="115" cy="115" r="104" class="track" />
        <circle
          cx="115"
          cy="115"
          r="104"
          class="fill"
          stroke-dasharray={ring}
          stroke-dashoffset={ring * (1 - percent / 100)}
        />
      </svg>
      <div class="figure">
        <div class="percent">{percent}%</div>
        <div class="caption">
          {phase === 'settling' ? 'adding it up' : phase === 'done' ? 'scan complete' : 'scanning inbox'}
        </div>
      </div>
    </div>

    <h1>
      {#if phase === 'done'}
        Read {n(summary?.scanned ?? fetched)} emails so you never have to.
      {:else if total === null}
        Looking for new mail…
      {:else if total === 0}
        No new mail since the last scan.
      {:else}
        Reading {n(total)} emails so you never have to.
      {/if}
    </h1>
    <p class="where">{source?.label ?? 'IMAP'} · nothing leaves this computer</p>

    {#if error?.kind === 'signInRefused'}
      <div class="problem-card">
        <SignInFailed host={error.host} serverSays={error.serverSays} guide={null} onRetry={() => goto('/connect')} />
      </div>
    {:else}
      <div class="stats">
        <div class="stat"><strong>{n(fetched)}</strong><span>emails scanned</span></div>
        <div class="stat"><strong>{n(senders)}</strong><span>senders found</span></div>
        <div class="stat"><strong class="acc">{n(subscriptions)}</strong><span>subscriptions so far</span></div>
        <div class="stat"><strong class="sage">{n(newsletters)}</strong><span>newsletters so far</span></div>
      </div>

      {#if error}
        <p class="note" role="alert">
          {#if error.kind === 'cancelled'}
            Scan cancelled. Everything read so far is kept.
          {:else if error.kind === 'unreachable'}
            The mail server could not be reached: {error.message}
          {:else if error.kind === 'alreadyRunning'}
            Another scan is already running.
          {:else if error.kind === 'failed'}
            The scan stopped: {error.message}
          {/if}
        </p>
      {:else if latestFind}
        <p class="note"><span class="dot"></span>just found: {latestFind}</p>
      {/if}

      {#if phase === 'fetching'}
        <button type="button" class="outline" onclick={() => commands.cancelScan()}>Cancel scan</button>
      {:else if phase === 'failed'}
        <button type="button" class="outline" onclick={start}>Resume scan</button>
      {:else if phase === 'done'}
        <button type="button" class="outline" onclick={() => goto('/')}>Done</button>
      {/if}
    {/if}
  </section>
</main>

<style>
  main {
    min-height: 100vh;
    padding: 1.75rem 3rem;
    box-sizing: border-box;
  }

  section {
    display: flex;
    flex-direction: column;
    align-items: center;
    margin-top: 4rem;
    text-align: center;
  }

  .ring {
    position: relative;
    width: 14.375rem;
    height: 14.375rem;
  }

  .ring svg {
    width: 100%;
    height: 100%;
    transform: rotate(-90deg);
  }

  circle {
    fill: none;
    stroke-width: 14;
  }

  .track {
    stroke: var(--card2);
  }

  .fill {
    stroke: var(--acc);
    stroke-linecap: round;
    transition: stroke-dashoffset 0.3s;
  }

  .figure {
    position: absolute;
    inset: 0;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
  }

  .percent {
    font-family: var(--font-heading);
    font-weight: var(--font-heading-weight);
    font-size: 2.875rem;
    color: var(--hd);
  }

  .caption {
    font-size: 0.8125rem;
    color: var(--mut);
  }

  h1 {
    margin: 1.875rem 0 0;
    font-family: var(--font-heading);
    font-weight: var(--font-heading-weight);
    font-size: 1.625rem;
    color: var(--hd);
  }

  .where {
    margin: var(--space-2) 0 0;
    font-size: 0.875rem;
    color: var(--mut);
  }

  .stats {
    display: flex;
    flex-wrap: wrap;
    justify-content: center;
    gap: 1.125rem;
    margin-top: 2.375rem;
  }

  .stat {
    width: 12.5rem;
    box-sizing: border-box;
    text-align: left;
    background: var(--card);
    border: var(--stroke) solid var(--line);
    border-radius: var(--radius-lg);
    padding: 1.125rem 1.375rem;
    box-shadow: var(--shadow-sm);
  }

  .stat strong {
    display: block;
    font-family: var(--font-heading);
    font-weight: var(--font-heading-weight);
    font-size: 1.875rem;
    color: var(--hd);
  }

  .stat .acc {
    color: var(--accDeep);
  }

  .stat .sage {
    color: var(--sageDeep);
  }

  .stat span {
    display: block;
    margin-top: 0.125rem;
    font-size: 0.8125rem;
    color: var(--mut);
  }

  .note {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    margin: 2.125rem 0 0;
    font-size: 0.84rem;
    color: var(--mut);
    background: var(--card2);
    padding: var(--space-2) 1.125rem;
    border-radius: var(--pill);
  }

  .dot {
    width: 0.4375rem;
    height: 0.4375rem;
    border-radius: var(--pill);
    background: var(--dotOk);
  }

  .outline {
    margin-top: 1.875rem;
    background: transparent;
    border: var(--stroke-strong) solid var(--line);
    color: var(--mut);
    font-family: var(--font-body);
    font-size: 0.9rem;
    font-weight: 600;
    padding: var(--space-3) 1.625rem;
    border-radius: var(--pill);
    cursor: pointer;
  }

  .outline:hover {
    border-color: var(--mut);
  }

  .outline:active {
    background: var(--card2);
  }

  .problem-card {
    margin-top: 2.375rem;
    width: min(100%, 28rem);
  }
</style>
