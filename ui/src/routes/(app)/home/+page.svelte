<script lang="ts">
  import { goto } from '$app/navigation';
  import { CircleCheck, Layers } from 'lucide-svelte';
  import { commands, type Dashboard, type SourceSummary } from '$lib/bindings';
  import Bubbles from '$lib/components/Bubbles.svelte';
  import Button from '$lib/components/Button.svelte';
  import Empty from '$lib/components/Empty.svelte';
  import { ago, count, money, rate } from '$lib/format';
  import { sweep } from '$lib/sweep.svelte';

  let dashboard = $state<Dashboard | null>(null);
  let sources = $state<SourceSummary[]>([]);
  let problem = $state<string | null>(null);

  $effect(() => {
    void sweep.version;
    commands.dashboard().then((r) => {
      if (r.status === 'ok') dashboard = r.data;
      else problem = r.error;
    });
    commands.listSources().then((r) => r.status === 'ok' && (sources = r.data));
  });

  let headline = $derived(dashboard?.monthlySpend[0] ?? null);
  let others = $derived(dashboard?.monthlySpend.slice(1) ?? []);
  let saving = $derived(
    dashboard?.cancelAll.find((a) => a.currency === headline?.currency) ?? null,
  );

  /** Every newsletter still subscribed, the figure on the button. */
  async function unsubscribeAll() {
    const list = await commands.newsletters();
    if (list.status === 'error') {
      problem = list.error;
      return;
    }
    await sweep.begin(
      list.data
        .filter((n) => n.unsubscribedAt === null)
        .map((n) => ({ kind: 'sender', id: n.id }) as const),
    );
  }

  function scanAgain() {
    goto(sources.length === 1 ? `/scan?source=${sources[0].id}` : '/welcome');
  }
</script>

<!-- S03 — Dashboard -->
<header>
  <h1>Home</h1>
  {#if dashboard && dashboard.sources > 0}
    <p class="status">
      <span class="dot"></span>
      Local-only · {dashboard.sources}
      {dashboard.sources === 1 ? 'source' : 'sources'} · scanned {ago(dashboard.lastSyncAt)}
      <button type="button" class="link" onclick={scanAgain}>Scan again</button>
    </p>
  {/if}
</header>

{#if problem}
  <p class="problem" role="alert">{problem}</p>
{:else if dashboard && dashboard.sources === 0}
  <Empty title="Nothing here yet">
    {#snippet icon()}<Layers size={26} strokeWidth={2.75} />{/snippet}
    Connect a source and run a scan — your dashboard fills itself.
    {#snippet action()}
      <Button size="small" onclick={() => goto('/welcome')}>Add a source</Button>
    {/snippet}
  </Empty>
{:else if dashboard}
  <div class="page">
    <section class="summary">
      <h2>Your subscriptions,<br />seen as they are.</h2>
      <p class="lede">
        Each circle is a service, sized by what it costs you. Open one, or terminate the lot.
      </p>

      <div class="figures">
        <div>
          <div class="figure">{headline ? money(headline, { whole: true }) : '—'}</div>
          <div class="caption">
            per month · {count(dashboard.subscriptions)}
            {dashboard.subscriptions === 1 ? 'subscription' : 'subscriptions'}
          </div>
          {#if others.length > 0}
            <div class="caption">plus {others.map((a) => money(a, { whole: true })).join(' · ')}</div>
          {/if}
        </div>
        <div>
          <div class="figure">{count(dashboard.emailsPerYear)}</div>
          <div class="caption">
            emails / year · {count(dashboard.newsletters)}
            {dashboard.newsletters === 1 ? 'newsletter' : 'newsletters'}
          </div>
        </div>
      </div>

      <!-- Cancelling arrives with playbooks in M4; unsubscribing runs now. -->
      {#if dashboard.subscriptions > 0 || dashboard.unsubscribeAll > 0}
      <div class="actions">
        {#if saving}
        <Button disabled title="Arrives with cancellation playbooks">
          Cancel all — save {money(saving, { whole: true })}/mo
        </Button>
        {/if}
        <Button variant="outline" disabled={dashboard.unsubscribeAll === 0 || sweep.running} onclick={unsubscribeAll}>
          Unsubscribe all — cut {count(dashboard.unsubscribeAll)}/yr
        </Button>
      </div>
      {/if}

      {#if dashboard.loudest.length > 0}
        <h3>Loudest senders</h3>
        <div class="chips">
          {#each dashboard.loudest as loud (loud.senderId)}
            <button
              type="button"
              class="chip"
              onclick={() => goto(`/newsletters?q=${encodeURIComponent(loud.name)}`)}
            >
              {loud.name} · {rate(loud.lastYear)}
            </button>
          {/each}
        </div>
      {/if}
    </section>

    <section class="bubbles">
      {#if dashboard.subscriptions === 0}
        <Empty title="A clean inbox. Honestly.">
          {#snippet icon()}<CircleCheck size={26} strokeWidth={2.75} />{/snippet}
          We scanned {count(dashboard.scanned)} emails and found no paid subscriptions. Newsletters
          live in their own tab.
        </Empty>
      {:else}
        <Bubbles tiles={dashboard.services} currency={headline?.currency ?? null} />
      {/if}
    </section>
  </div>
{/if}

<style>
  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--space-4);
    flex-wrap: wrap;
  }

  h1 {
    margin: 0;
    font-family: var(--font-heading);
    font-weight: var(--font-heading-weight);
    font-size: 1.0625rem;
    color: var(--hd);
  }

  .status {
    margin: 0;
    display: flex;
    align-items: center;
    gap: var(--space-2);
    font-size: 0.8125rem;
    color: var(--mut);
  }

  .dot {
    width: 0.4375rem;
    height: 0.4375rem;
    border-radius: var(--pill);
    background: var(--dotOk);
  }

  .link {
    border: none;
    background: none;
    padding: 0;
    font: inherit;
    font-weight: 700;
    color: var(--accDeep);
    cursor: pointer;
  }

  .link:hover {
    opacity: 0.8;
  }

  .page {
    display: flex;
    gap: 2.5rem;
    align-items: flex-start;
    flex-wrap: wrap;
  }

  .summary {
    flex: 0 1 34rem;
    margin: 4.5rem 0 0 0.75rem;
  }

  h2 {
    margin: 0;
    font-family: var(--font-heading);
    font-weight: var(--font-heading-weight);
    font-size: 2.625rem;
    line-height: 1.14;
    color: var(--hd);
  }

  .lede {
    margin: 0.875rem 0 0;
    max-width: 26.25rem;
    font-size: 0.97rem;
    line-height: 1.55;
    color: var(--mut);
  }

  .figures {
    display: flex;
    gap: 2.125rem;
    margin-top: 2.125rem;
  }

  .figure {
    font-family: var(--font-heading);
    font-weight: var(--font-heading-weight);
    font-size: 3.125rem;
    color: var(--hd);
  }

  .caption {
    font-size: 0.8125rem;
    color: var(--mut);
  }

  .actions {
    display: flex;
    flex-wrap: wrap;
    gap: 0.875rem;
    margin-top: 2.25rem;
  }

  h3 {
    margin: 2.75rem 0 0;
    font-size: 0.78rem;
    font-weight: 700;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--mut);
  }

  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-2);
    margin-top: 0.625rem;
  }

  .chip {
    border: none;
    cursor: pointer;
    background: var(--sageSoft);
    color: var(--sageDeep);
    font-family: var(--font-body);
    font-size: 0.78rem;
    font-weight: 600;
    padding: 0.375rem 0.875rem;
    border-radius: var(--pill);
  }

  .chip:hover {
    filter: brightness(0.97);
  }

  .bubbles {
    flex: 1 1 24rem;
    max-width: 38.75rem;
    margin-top: 2.75rem;
  }

  .problem {
    color: var(--bad);
  }
</style>
