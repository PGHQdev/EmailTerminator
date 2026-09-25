<script lang="ts">
  import { goto } from '$app/navigation';
  import { openUrl } from '@tauri-apps/plugin-opener';
  import { TriangleAlert } from 'lucide-svelte';
  import Button from '$lib/components/Button.svelte';
  import Check from '$lib/components/Check.svelte';
  import { evidence } from '$lib/evidence.svelte';
  import { count } from '$lib/format';
  import { sweep, type Row } from '$lib/sweep.svelte';

  // S11 — Bulk action review, then the run with a result per row.
  $effect(() => {
    if (sweep.rows.length === 0) goto('/home');
  });

  let listed = $derived(sweep.rows.filter((r) => r.included || !r.critical));
  let excluded = $derived(sweep.rows.filter((r) => r.critical && !r.included));
  let included = $derived(sweep.rows.filter((r) => r.included));
  let finished = $derived(included.filter((r) => r.state === 'done').length);
  let removed = $derived(included.reduce((t, r) => t + r.emailsPerYear, 0));
  let started = $derived(sweep.rows.some((r) => r.state !== 'queued'));
  let complete = $derived(included.length > 0 && finished === included.length);

  const routeNote: Record<Row['route'], string> = {
    oneClick: '',
    page: ' · page only',
    mail: ' · email only',
    nothing: ' · no header',
  };

  function status(row: Row): { text: string; tone: string } {
    if (!row.included) return { text: 'kept', tone: 'mut' };
    if (row.state === 'queued') return { text: 'queued', tone: 'mut' };
    if (row.state === 'running') return { text: 'running…', tone: 'run' };
    const outcome = row.result?.outcome;
    if (outcome === 'succeeded') return { text: 'done', tone: 'ok' };
    if (outcome === 'needsYou') return { text: 'needs you', tone: 'run' };
    return { text: 'failed', tone: 'bad' };
  }
</script>

<!-- S11 — Bulk action review -->
<header>
  <div>
    <h1>Review the sweep</h1>
    <p>
      {sweep.rows.length}
      {sweep.rows.length === 1 ? 'target' : 'targets'} · uncheck anything you want to keep
    </p>
  </div>
  <div class="figure">
    <strong>{count(removed)}</strong>
    <span>emails / year removed</span>
  </div>
</header>

<div class="layout">
  <section class="list">
    <div class="bar">
      <span>
        {#if complete}Finished · {finished} of {included.length} done
        {:else if started}Executing · {finished} of {included.length} done
        {:else}Ready · {included.length} to run{/if}
      </span>
      <div class="track">
        <div class="fill" style:width="{included.length ? (finished / included.length) * 100 : 0}%"></div>
      </div>
    </div>
    {#each listed as row (row.target.kind + row.target.id)}
      {@const s = status(row)}
      {@const r = row.result}
      <div class="row" class:faded={row.state === 'done'} class:kept={!row.included}>
        <Check
          checked={row.included}
          onchange={() => sweep.toggle(row)}
          label={`${row.included ? 'Keep' : 'Include'} ${row.name}`}
        />
        <span class="name">{row.name}</span>
        <span class="mut">
          {row.target.kind === 'sender' ? 'newsletter' : 'subscription'}{routeNote[row.route]}
        </span>
        <span class="mono">−{count(row.emailsPerYear)}/yr</span>
        <span class="status {s.tone}" title={r?.detail}>
          <span class="dot"></span>{s.text}
          {#if r && r.outcome === 'needsYou' && r.finishAt}
            {@const at = r.finishAt}
            · <button type="button" onclick={() => openUrl(at)}>
              {at.toLowerCase().startsWith('mailto:') ? 'write the email' : 'open page'}
            </button>
          {:else if r && r.actionIds.length > 0}
            · <button type="button" onclick={() => evidence.open(r.actionIds[0])}>
              {r.outcome === 'succeeded' ? 'evidence' : 'why'}
            </button>
          {:else if r}
            · {r.detail}
          {/if}
        </span>
      </div>
    {/each}
  </section>

  <aside>
    {#if excluded.length > 0}
      <div class="critical">
        <h2>
          <TriangleAlert size={16} strokeWidth={2.75} />
          {excluded.length} critical {excluded.length === 1 ? 'service' : 'services'} excluded
        </h2>
        <ul>
          {#each excluded as row (row.target.kind + row.target.id)}
            <li>
              <span>{row.name}</span>
              <button type="button" disabled={sweep.running} onclick={() => sweep.toggle(row)}>
                include
              </button>
            </li>
          {/each}
        </ul>
        <p>Including one triggers its own confirmation.</p>
      </div>
    {/if}
    <div class="card">
      <p>
        One-click unsubscribes run straight away. A sender without one asks you to finish on its
        page or by email, and the row links there.
      </p>
      {#if sweep.running}
        <Button variant="outline" size="medium" disabled={sweep.stopping} onclick={() => sweep.stop()}>
          {sweep.stopping ? 'Pausing…' : 'Pause sweep'}
        </Button>
      {:else if complete}
        <Button variant="dark" size="medium" onclick={() => goto('/home')}>Done</Button>
      {:else}
        <Button size="medium" disabled={included.length === 0} onclick={() => sweep.run()}>
          {started ? 'Resume sweep' : 'Start sweep'}
        </Button>
      {/if}
      {#if sweep.problem}<p class="problem" role="alert">{sweep.problem}</p>{/if}
    </div>
  </aside>
</div>

<style>
  header {
    display: flex;
    align-items: flex-end;
    justify-content: space-between;
    flex-wrap: wrap;
    gap: var(--space-3);
  }

  h1 {
    margin: 0;
    font-family: var(--font-heading);
    font-weight: var(--font-heading-weight);
    font-size: 1.875rem;
    color: var(--hd);
  }

  header p {
    margin: 0.25rem 0 0;
    font-size: 0.875rem;
    color: var(--mut);
  }

  .figure {
    text-align: right;
  }

  .figure strong {
    display: block;
    font-family: var(--font-heading);
    font-weight: var(--font-heading-weight);
    font-size: 2rem;
    color: var(--sageDeep);
  }

  .figure span {
    font-size: 0.78rem;
    color: var(--mut);
  }

  .layout {
    display: flex;
    gap: 1.25rem;
    margin-top: 1.25rem;
    align-items: flex-start;
  }

  .list {
    flex: 1;
    min-width: 0;
    background: var(--card);
    border: var(--stroke) solid var(--line);
    border-radius: var(--radius-lg);
    box-shadow: var(--shadow-sm);
    overflow: hidden;
  }

  .bar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 1rem;
    padding: 0.875rem 1.375rem;
    border-bottom: var(--stroke) solid var(--line);
    font-size: 0.75rem;
    font-weight: 700;
    letter-spacing: 0.07em;
    text-transform: uppercase;
    color: var(--mut);
  }

  .track {
    width: 13.75rem;
    height: 0.4375rem;
    border-radius: var(--pill);
    background: var(--card2);
    overflow: hidden;
  }

  .fill {
    height: 100%;
    background: var(--sage);
    border-radius: var(--pill);
    transition: width 0.3s;
  }

  .row {
    display: grid;
    grid-template-columns: 2.75rem 1.5fr 1fr 0.8fr 1.3fr;
    align-items: center;
    padding: 0.6875rem 1.375rem;
    font-size: 0.844rem;
    border-bottom: var(--stroke) solid var(--line);
  }

  .row:last-child {
    border-bottom: none;
  }

  .faded {
    opacity: 0.66;
  }

  .kept {
    opacity: 0.55;
  }

  .name {
    font-weight: 600;
    color: var(--hd);
  }

  .mut {
    color: var(--mut);
  }

  .mono {
    font-family: var(--font-mono);
    font-size: 0.78rem;
  }

  .status {
    display: flex;
    align-items: center;
    gap: 0.4375rem;
    font-size: 0.78rem;
    font-weight: 700;
    min-width: 0;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .status .dot {
    width: 0.4375rem;
    height: 0.4375rem;
    flex: none;
    border-radius: var(--pill);
    background: var(--line);
  }

  .status.ok {
    color: var(--ok);
  }

  .status.ok .dot {
    background: var(--dotOk);
  }

  .status.run {
    color: var(--accDeep);
  }

  .status.run .dot,
  .status.bad .dot {
    background: var(--acc);
  }

  .status.bad {
    color: var(--bad);
  }

  .status button,
  .critical button {
    padding: 0;
    border: none;
    background: none;
    font: inherit;
    font-weight: 700;
    color: var(--accDeep);
    cursor: pointer;
  }

  .status button:hover,
  .critical button:hover {
    opacity: 0.8;
  }

  aside {
    width: 20.625rem;
    flex: none;
    display: flex;
    flex-direction: column;
    gap: 0.75rem;
  }

  .critical {
    background: var(--accSoft);
    border-radius: var(--radius-lg);
    padding: 1.25rem 1.375rem;
  }

  .critical h2 {
    margin: 0;
    display: flex;
    align-items: center;
    gap: 0.5625rem;
    font-size: 0.875rem;
    font-weight: 800;
    color: var(--accDeep);
  }

  .critical ul {
    list-style: none;
    margin: 0.75rem 0 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
    font-size: 0.844rem;
  }

  .critical li {
    display: flex;
    justify-content: space-between;
    align-items: center;
  }

  .critical button {
    font-size: 0.75rem;
  }

  .critical button:disabled {
    opacity: 0.45;
    cursor: default;
  }

  .critical p {
    margin: 0.75rem 0 0;
    font-size: 0.75rem;
    color: var(--accDeep);
  }

  .card {
    background: var(--card);
    border: var(--stroke) solid var(--line);
    border-radius: var(--radius-lg);
    box-shadow: var(--shadow-sm);
    padding: 1.25rem 1.375rem;
    display: flex;
    flex-direction: column;
    gap: 0.875rem;
  }

  .card p {
    margin: 0;
    font-size: 0.8125rem;
    line-height: 1.55;
    color: var(--mut);
  }

  .card .problem {
    color: var(--bad);
  }
</style>
