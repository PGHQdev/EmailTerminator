<script lang="ts">
  import { goto } from '$app/navigation';
  import { page } from '$app/state';
  import { ChevronDown, CircleCheck, Rows3, Shapes } from 'lucide-svelte';
  import { commands, type Subscription } from '$lib/bindings';
  import Bubbles from '$lib/components/Bubbles.svelte';
  import Check from '$lib/components/Check.svelte';
  import Chip from '$lib/components/Chip.svelte';
  import Empty from '$lib/components/Empty.svelte';
  import Marker from '$lib/components/Marker.svelte';
  import SearchField from '$lib/components/SearchField.svelte';
  import { cadenceLabel, count, initials, money, shortDate, sum } from '$lib/format';

  let rows = $state<Subscription[] | null>(null);
  let problem = $state<string | null>(null);
  let query = $state(page.url.searchParams.get('q') ?? '');
  let filter = $state<'all' | 'price' | 'critical' | 'canceling'>('all');
  let sort = $state<'cost' | 'name' | 'last' | 'volume'>('cost');
  let view = $state<'table' | 'bubbles'>('table');
  let selected = $state(new Set<number>());

  $effect(() => {
    commands.subscriptions().then((r) => {
      if (r.status === 'ok') rows = r.data;
      else problem = r.error;
    });
  });

  // The currency most subscriptions bill in heads every total; there is no
  // conversion (PLAN.md Part 4).
  let currency = $derived.by(() => {
    const tally = new Map<string, number>();
    for (const r of rows ?? []) {
      if (r.monthly) tally.set(r.monthly.currency, (tally.get(r.monthly.currency) ?? 0) + 1);
    }
    return [...tally].sort((a, b) => b[1] - a[1])[0]?.[0] ?? null;
  });
  const monthly = (list: Subscription[]) =>
    currency ? sum(list.flatMap((r) => (r.monthly ? [r.monthly] : [])), currency) : null;

  const filters = {
    all: () => true,
    price: (r: Subscription) => r.priceIncrease,
    critical: (r: Subscription) => r.isCritical,
    canceling: (r: Subscription) => r.status === 'canceling',
  };
  const sorts: Record<typeof sort, { label: string; compare: (a: Subscription, b: Subscription) => number }> = {
    cost: {
      label: 'monthly cost',
      compare: (a, b) =>
        Number(b.monthly?.currency === currency) - Number(a.monthly?.currency === currency) ||
        (b.monthly?.minorUnits ?? -1) - (a.monthly?.minorUnits ?? -1),
    },
    name: { label: 'name', compare: (a, b) => a.name.localeCompare(b.name) },
    last: {
      label: 'last charge',
      compare: (a, b) => (b.lastChargeAt ?? '').localeCompare(a.lastChargeAt ?? ''),
    },
    volume: { label: 'emails per year', compare: (a, b) => b.emailsPerYear - a.emailsPerYear },
  };

  let shown = $derived(
    (rows ?? [])
      .filter(filters[filter])
      .filter((r) => r.name.toLowerCase().includes(query.trim().toLowerCase()))
      .sort(sorts[sort].compare),
  );
  let total = $derived(monthly(rows ?? []));
  let chosen = $derived((rows ?? []).filter((r) => selected.has(r.id)));
  let chosenCost = $derived(monthly(chosen));

  function toggle(id: number) {
    const next = new Set(selected);
    if (!next.delete(id)) next.add(id);
    selected = next;
  }

  const statusLabel = { active: 'Active', canceling: 'Canceling', canceled: 'Canceled' };
</script>

<!-- S04 — Subscriptions list -->
<header>
  <div class="title">
    <h1>Subscriptions</h1>
    {#if rows && total}
      <span>
        {rows.length} services · {money(total)}/mo ·
        {money({ ...total, minorUnits: total.minorUnits * 12 }, { whole: true })}/yr
      </span>
    {/if}
  </div>
  <div class="tools">
    <div class="segments" role="group" aria-label="View">
      <button type="button" class:on={view === 'table'} aria-pressed={view === 'table'} onclick={() => (view = 'table')}>
        <Rows3 size={13} strokeWidth={2.75} />Table
      </button>
      <button type="button" class:on={view === 'bubbles'} aria-pressed={view === 'bubbles'} onclick={() => (view = 'bubbles')}>
        <Shapes size={13} strokeWidth={2.75} />Bubbles
      </button>
    </div>
    <SearchField bind:value={query} placeholder="Search services…" />
  </div>
</header>

{#if problem}
  <p class="problem" role="alert">{problem}</p>
{:else if rows && rows.length === 0}
  <Empty title="A clean inbox. Honestly.">
    {#snippet icon()}<CircleCheck size={26} strokeWidth={2.75} />{/snippet}
    The last scan found no paid subscriptions. Newsletters live in their own tab.
  </Empty>
{:else if rows}
  <div class="filters">
    <span class="label">Filter</span>
    <Chip label="All" count={rows.length} on={filter === 'all'} onclick={() => (filter = 'all')} />
    <Chip label="Price increased" count={rows.filter(filters.price).length} on={filter === 'price'} onclick={() => (filter = 'price')} />
    <Chip label="Critical" count={rows.filter(filters.critical).length} on={filter === 'critical'} onclick={() => (filter = 'critical')} />
    <Chip label="Canceling" count={rows.filter(filters.canceling).length} on={filter === 'canceling'} onclick={() => (filter = 'canceling')} />
    <label class="sort">
      Sort:
      <select bind:value={sort}>
        {#each Object.entries(sorts) as [value, { label }] (value)}
          <option {value}>{label}</option>
        {/each}
      </select>
      <ChevronDown size={13} strokeWidth={2.75} />
    </label>
  </div>

  {#if view === 'bubbles'}
    <div class="bubbles"><Bubbles tiles={shown} {currency} /></div>
  {:else}
    <div class="table">
      <div class="row head">
        <span></span><span>Service</span><span>Per month</span><span>Cadence</span><span>Last charge</span
        ><span>Emails/yr</span><span>Status</span>
      </div>
      {#each shown as r (r.id)}
        <div
          class="row"
          class:selected={selected.has(r.id)}
          role="link"
          tabindex="0"
          onclick={() => goto(`/service?id=${r.id}`)}
          onkeydown={(e) => e.key === 'Enter' && goto(`/service?id=${r.id}`)}
        >
          <Check checked={selected.has(r.id)} onchange={() => toggle(r.id)} label={`Select ${r.name}`} />
          <span class="name">
            <span class="avatar">{initials(r.name)}</span>
            <strong>{r.name}</strong>
            {#if r.isCritical}<Marker kind="critical" />{/if}
            {#if r.priceIncrease}<Marker kind="price" label="price ↑" />{/if}
          </span>
          <span class="mono">{r.monthly ? money(r.monthly) : '—'}</span>
          <span class="mut">{cadenceLabel(r.cadence)}</span>
          <span class="mut">{shortDate(r.lastChargeAt)}</span>
          <span class="mono">{count(r.emailsPerYear)}</span>
          <span class="status {r.status}"><span class="dot"></span>{statusLabel[r.status]}</span>
        </div>
      {:else}
        <p class="none">No service matches.</p>
      {/each}
    </div>
  {/if}

  {#if chosen.length > 0}
    <p class="selection">
      {chosen.length} selected — {chosenCost ? money(chosenCost) : '—'}/mo,
      {count(chosen.reduce((t, r) => t + r.emailsPerYear, 0))} emails/yr
    </p>
  {/if}
{/if}

<style>
  header {
    display: flex;
    align-items: center;
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

  .tools {
    display: flex;
    align-items: center;
    gap: 0.75rem;
  }

  .segments {
    display: flex;
    background: var(--card2);
    border-radius: var(--pill);
    padding: 0.25rem;
  }

  .segments button {
    display: flex;
    align-items: center;
    gap: 0.4375rem;
    border: none;
    background: transparent;
    cursor: pointer;
    font-family: var(--font-body);
    font-size: 0.8125rem;
    font-weight: 600;
    color: var(--mut);
    padding: 0.4375rem 1rem;
    border-radius: var(--pill);
  }

  .segments button:hover {
    color: var(--fg);
  }

  .segments .on {
    background: var(--card);
    color: var(--hd);
    font-weight: 700;
    box-shadow: var(--shadow-sm);
  }

  .filters {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: var(--space-2);
    margin-top: 1.125rem;
  }

  .label {
    font-size: 0.75rem;
    font-weight: 700;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: var(--mut);
    margin-right: 0.25rem;
  }

  .sort {
    margin-left: auto;
    display: flex;
    align-items: center;
    gap: 0.375rem;
    font-size: 0.8125rem;
    color: var(--mut);
    position: relative;
  }

  .sort select {
    appearance: none;
    border: none;
    background: transparent;
    font: inherit;
    color: var(--mut);
    cursor: pointer;
    padding-right: 0.25rem;
  }

  .sort select:hover {
    color: var(--fg);
  }

  .table {
    margin-top: 1rem;
    background: var(--card);
    border: var(--stroke) solid var(--line);
    border-radius: var(--radius-lg);
    overflow: hidden;
    box-shadow: var(--shadow-sm);
  }

  .row {
    display: grid;
    grid-template-columns: 3.25rem 1.6fr 0.8fr 0.9fr 1fr 0.9fr 1fr;
    align-items: center;
    padding: 0.8125rem 1.375rem;
    font-size: 0.875rem;
    border-bottom: var(--stroke) solid var(--line);
    cursor: pointer;
  }

  .row:last-child {
    border-bottom: none;
  }

  .row:hover {
    background: var(--card2);
  }

  .row.selected {
    background: var(--sageSoft);
  }

  .row.head {
    padding: 0.75rem 1.375rem;
    font-size: 0.72rem;
    font-weight: 700;
    letter-spacing: 0.07em;
    text-transform: uppercase;
    color: var(--mut);
    border-bottom: var(--stroke) solid var(--line);
    cursor: default;
  }

  .row.head:hover {
    background: transparent;
  }

  .name {
    display: flex;
    align-items: center;
    gap: 0.6875rem;
    min-width: 0;
  }

  .name strong {
    font-weight: 600;
    color: var(--hd);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .avatar {
    width: 1.875rem;
    height: 1.875rem;
    flex: none;
    border-radius: var(--pill);
    background: var(--sageSoft);
    color: var(--sageDeep);
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: 0.72rem;
    font-weight: 800;
  }

  .row.selected .avatar {
    background: var(--card);
  }

  .mono {
    font-family: var(--font-mono);
    font-size: 0.84rem;
  }

  .mut {
    color: var(--mut);
  }

  .status {
    display: flex;
    align-items: center;
    gap: 0.4375rem;
    font-size: 0.8125rem;
    font-weight: 600;
    color: var(--ok);
  }

  .status .dot {
    width: 0.4375rem;
    height: 0.4375rem;
    border-radius: var(--pill);
    background: var(--dotOk);
  }

  .status.canceling {
    color: var(--accDeep);
  }

  .status.canceling .dot {
    background: var(--acc);
  }

  .status.canceled {
    color: var(--mut);
  }

  .status.canceled .dot {
    background: var(--mut);
  }

  .none {
    margin: 0;
    padding: 1rem 1.375rem;
    font-size: 0.8125rem;
    color: var(--mut);
  }

  .bubbles {
    max-width: 38.75rem;
    margin: 1rem auto 0;
  }

  .selection {
    position: sticky;
    bottom: 1rem;
    margin: 1.25rem 0 0;
    font-size: 0.8125rem;
    color: var(--mut);
  }

  .problem {
    color: var(--bad);
  }
</style>
