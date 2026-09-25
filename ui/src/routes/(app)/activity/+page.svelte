<script lang="ts">
  import { Clock } from 'lucide-svelte';
  import { commands, type Entry } from '$lib/bindings';
  import { actionLabel, filters, outcomeLabel, tone, type Filter } from '$lib/activity';
  import Chip from '$lib/components/Chip.svelte';
  import Empty from '$lib/components/Empty.svelte';
  import { evidence } from '$lib/evidence.svelte';
  import { when } from '$lib/format';
  import { sweep } from '$lib/sweep.svelte';

  let rows = $state<Entry[] | null>(null);
  let problem = $state<string | null>(null);
  let filter = $state<Filter>('all');

  $effect(() => {
    void sweep.version;
    commands.activity().then((r) => {
      if (r.status === 'ok') rows = r.data;
      else problem = r.error;
    });
  });

  let shown = $derived((rows ?? []).filter(filters[filter].keep));

  function link(entry: Entry): string {
    if (entry.kind === 'sync') return 'sync report';
    return entry.evidence ? 'email' : 'attempt log';
  }
</script>

<!-- S14 — Activity log -->
<header>
  <div class="title">
    <h1>Activity</h1>
    <span>every action the app took, with receipts</span>
  </div>
  {#if rows && rows.length > 0}
    <div class="chips">
      {#each Object.entries(filters) as [key, f] (key)}
        <Chip
          label={f.label}
          count={rows.filter(f.keep).length}
          on={filter === key}
          onclick={() => (filter = key as Filter)}
        />
      {/each}
    </div>
  {/if}
</header>

{#if problem}
  <p class="problem" role="alert">{problem}</p>
{:else if rows && rows.length === 0}
  <Empty title="No actions yet" muted>
    {#snippet icon()}<Clock size={26} strokeWidth={2.75} />{/snippet}
    Everything the app does on your behalf shows up here, with evidence.
  </Empty>
{:else if rows}
  <div class="table">
    <div class="row head">
      <span>Action</span><span>Target</span><span>When</span><span>Outcome</span><span>Evidence</span>
    </div>
    {#each shown as entry (entry.id)}
      <div class="row">
        <span class="action"><span class="dot {tone(entry)}"></span>{actionLabel(entry)}</span>
        <span class="mut">{entry.target}</span>
        <span class="mono">{when(entry.at)}</span>
        <span class="outcome {tone(entry)}" title={entry.detail}>{outcomeLabel(entry)}</span>
        <button type="button" class="link" onclick={() => evidence.show(entry)}>{link(entry)}</button>
      </div>
    {:else}
      <p class="none">Nothing here for this filter.</p>
    {/each}
  </div>
{/if}

<style>
  header {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    flex-wrap: wrap;
    gap: var(--space-3);
  }

  .title {
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

  .title span {
    font-size: 0.875rem;
    color: var(--mut);
  }

  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-2);
  }

  .table {
    margin-top: 1.25rem;
    background: var(--card);
    border: var(--stroke) solid var(--line);
    border-radius: var(--radius-lg);
    box-shadow: var(--shadow-sm);
    overflow: hidden;
  }

  .row {
    display: grid;
    grid-template-columns: 1.7fr 1.1fr 1fr 1fr 1fr;
    align-items: center;
    gap: 0.75rem;
    padding: 0.8125rem 1.5rem;
    font-size: 0.844rem;
    border-bottom: var(--stroke) solid var(--line);
  }

  .row:last-child {
    border-bottom: none;
  }

  .row:not(.head):hover {
    background: var(--card2);
  }

  .head {
    padding: 0.75rem 1.5rem;
    font-size: 0.72rem;
    font-weight: 700;
    letter-spacing: 0.07em;
    text-transform: uppercase;
    color: var(--mut);
  }

  .action {
    display: flex;
    align-items: center;
    gap: 0.625rem;
    font-weight: 600;
    color: var(--hd);
  }

  .dot {
    width: 0.5rem;
    height: 0.5rem;
    flex: none;
    border-radius: var(--pill);
    background: var(--line);
  }

  .dot.ok {
    background: var(--dotOk);
  }

  .dot.bad {
    background: var(--acc);
  }

  .mut {
    color: var(--mut);
  }

  .mono {
    font-family: var(--font-mono);
    font-size: 0.78rem;
    color: var(--mut);
  }

  .outcome {
    font-size: 0.78rem;
    font-weight: 700;
    color: var(--mut);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .outcome.ok {
    color: var(--ok);
  }

  .outcome.bad {
    color: var(--bad);
  }

  .link {
    justify-self: start;
    padding: 0;
    border: none;
    background: none;
    font-family: var(--font-body);
    font-size: 0.78rem;
    font-weight: 700;
    color: var(--accDeep);
    cursor: pointer;
  }

  .link:hover {
    opacity: 0.8;
  }

  .none {
    margin: 0;
    padding: 1rem 1.5rem;
    font-size: 0.875rem;
    color: var(--mut);
  }

  .problem {
    color: var(--bad);
  }
</style>
