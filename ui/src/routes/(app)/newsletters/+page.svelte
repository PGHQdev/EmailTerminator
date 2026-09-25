<script lang="ts">
  import { goto } from '$app/navigation';
  import { page } from '$app/state';
  import { ChevronDown, MailX } from 'lucide-svelte';
  import { commands, type Newsletter } from '$lib/bindings';
  import Check from '$lib/components/Check.svelte';
  import Chip from '$lib/components/Chip.svelte';
  import Empty from '$lib/components/Empty.svelte';
  import SearchField from '$lib/components/SearchField.svelte';
  import { count, frequency, initials } from '$lib/format';

  let rows = $state<Newsletter[] | null>(null);
  let problem = $state<string | null>(null);
  let query = $state(page.url.searchParams.get('q') ?? '');
  let filter = $state<'all' | 'daily' | 'oneClick'>('all');
  let sort = $state<'volume' | 'frequency' | 'name'>('volume');
  let selected = $state(new Set<number>());

  $effect(() => {
    commands.newsletters().then((r) => {
      if (r.status === 'ok') rows = r.data;
      else problem = r.error;
    });
  });

  const filters = {
    all: () => true,
    daily: (r: Newsletter) => (r.perWeek ?? 0) >= 5.5 && r.received >= 2,
    oneClick: (r: Newsletter) => r.oneClick,
  };
  const sorts: Record<typeof sort, { label: string; compare: (a: Newsletter, b: Newsletter) => number }> = {
    volume: { label: 'volume', compare: (a, b) => b.lastYear - a.lastYear || b.received - a.received },
    frequency: { label: 'frequency', compare: (a, b) => (b.perWeek ?? 0) - (a.perWeek ?? 0) },
    name: { label: 'name', compare: (a, b) => a.name.localeCompare(b.name) },
  };

  let shown = $derived.by(() => {
    const q = query.trim().toLowerCase();
    return (rows ?? [])
      .filter(filters[filter])
      .filter((r) => r.name.toLowerCase().includes(q) || r.address.toLowerCase().includes(q))
      .sort(sorts[sort].compare);
  });
  let yearly = $derived((rows ?? []).reduce((t, r) => t + r.lastYear, 0));
  let chosen = $derived((rows ?? []).filter((r) => selected.has(r.id)));
  let chosenOneClick = $derived(chosen.filter((r) => r.oneClick).length);

  function toggle(id: number) {
    const next = new Set(selected);
    if (!next.delete(id)) next.add(id);
    selected = next;
  }

  // A sender that mails for a service opens that service (S06); the rest
  // have no detail of their own yet.
  const open = (r: Newsletter) => r.serviceId !== null && goto(`/service?id=${r.serviceId}`);
</script>

<!-- S05 — Newsletters list -->
<header>
  <div class="title">
    <h1>Newsletters</h1>
    {#if rows}
      <span>
        {count(rows.length)} senders · {count(yearly)} emails/yr ·
        {count(rows.filter(filters.oneClick).length)} support one-click unsubscribe
      </span>
    {/if}
  </div>
  <SearchField bind:value={query} placeholder="Search senders…" />
</header>

{#if problem}
  <p class="problem" role="alert">{problem}</p>
{:else if rows && rows.length === 0}
  <Empty title="No newsletters" muted>
    {#snippet icon()}<MailX size={26} strokeWidth={2.75} />{/snippet}
    The last scan found no list mail. Subscriptions live in their own tab.
  </Empty>
{:else if rows}
  <div class="filters">
    <span class="label">Filter</span>
    <Chip label="All" count={rows.length} on={filter === 'all'} onclick={() => (filter = 'all')} />
    <Chip label="Daily+" count={rows.filter(filters.daily).length} on={filter === 'daily'} onclick={() => (filter = 'daily')} />
    <Chip label="One-click" count={rows.filter(filters.oneClick).length} on={filter === 'oneClick'} onclick={() => (filter = 'oneClick')} />
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

  <div class="table">
    <div class="row head">
      <span></span><span>Sender</span><span>Frequency</span><span>Received</span><span>Unsubscribe</span
      ><span>Status</span>
    </div>
    {#each shown as r (r.id)}
      <!-- A row takes focus only when it is a link to its service. -->
      <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
      <div
        class="row"
        class:selected={selected.has(r.id)}
        class:opens={r.serviceId !== null}
        role={r.serviceId !== null ? 'link' : undefined}
        tabindex={r.serviceId !== null ? 0 : undefined}
        onclick={() => open(r)}
        onkeydown={(e) => e.key === 'Enter' && open(r)}
      >
        <Check checked={selected.has(r.id)} onchange={() => toggle(r.id)} label={`Select ${r.name}`} />
        <span class="name">
          <span class="avatar">{initials(r.name)}</span>
          <span class="who"><strong>{r.name}</strong><small>{r.address}</small></span>
        </span>
        <span class="mut">{frequency(r.perWeek, r.received)}</span>
        <span class="mono">{count(r.received)}</span>
        <span class="unsub" class:ok={r.oneClick}><span class="dot"></span>{r.oneClick ? 'One-click' : 'Manual only'}</span>
        <!-- Unsubscribing arrives in M3; until then every sender is subscribed. -->
        <span class="status">Subscribed</span>
      </div>
    {:else}
      <p class="none">No sender matches.</p>
    {/each}
  </div>

  {#if chosen.length > 0}
    <p class="selection">
      {chosen.length} selected — {count(chosen.reduce((t, r) => t + r.lastYear, 0))} emails/yr,
      {chosenOneClick === chosen.length ? 'all one-click' : `${chosenOneClick} one-click`}
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
  }

  .sort select {
    appearance: none;
    border: none;
    background: transparent;
    font: inherit;
    color: var(--mut);
    cursor: pointer;
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
    grid-template-columns: 3.25rem 1.7fr 1fr 0.9fr 1.2fr 1fr;
    align-items: center;
    padding: 0.8125rem 1.375rem;
    font-size: 0.875rem;
    border-bottom: var(--stroke) solid var(--line);
  }

  .row:last-child {
    border-bottom: none;
  }

  .row.opens {
    cursor: pointer;
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

  .avatar {
    width: 1.875rem;
    height: 1.875rem;
    flex: none;
    border-radius: var(--pill);
    background: var(--accSoft);
    color: var(--accDeep);
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: 0.72rem;
    font-weight: 800;
  }

  .who {
    min-width: 0;
  }

  .who strong,
  .who small {
    display: block;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .who strong {
    font-weight: 600;
    color: var(--hd);
  }

  .who small {
    font-size: 0.75rem;
    color: var(--mut);
  }

  .mono {
    font-family: var(--font-mono);
    font-size: 0.84rem;
  }

  .mut {
    color: var(--mut);
  }

  .unsub {
    display: flex;
    align-items: center;
    gap: 0.375rem;
    font-size: 0.78rem;
    font-weight: 700;
    color: var(--mut);
  }

  .unsub .dot {
    width: 0.4375rem;
    height: 0.4375rem;
    border-radius: var(--pill);
    background: var(--line);
  }

  .unsub.ok {
    color: var(--ok);
  }

  .unsub.ok .dot {
    background: var(--dotOk);
  }

  .status {
    font-size: 0.8125rem;
    font-weight: 600;
  }

  .none {
    margin: 0;
    padding: 1rem 1.375rem;
    font-size: 0.8125rem;
    color: var(--mut);
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
