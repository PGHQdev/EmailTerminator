<script lang="ts">
  import { page } from '$app/state';
  import { ArrowLeft, ShieldAlert } from 'lucide-svelte';
  import { commands, type ServiceDetail } from '$lib/bindings';
  import Button from '$lib/components/Button.svelte';
  import Marker from '$lib/components/Marker.svelte';
  import { count, initials, longDate, money, monthLabel, shortDate } from '$lib/format';
  import { priceRise, stepLine } from '$lib/history';

  let detail = $state<ServiceDetail | null | undefined>(undefined);
  let problem = $state<string | null>(null);

  $effect(() => {
    const id = Number(page.url.searchParams.get('id'));
    commands.serviceDetail(id).then((r) => {
      if (r.status === 'ok') detail = r.data;
      else problem = r.error;
    });
  });

  let currency = $derived(detail?.monthly?.currency ?? detail?.lastCharge?.amount.currency ?? null);
  let chart = $derived(detail && currency ? stepLine(detail.spend, currency) : null);
  let rise = $derived(detail ? priceRise(detail.priceChanges, Date.now()) : null);
  let maxBar = $derived(Math.max(1, ...(detail?.volume.map((v) => v.messages) ?? [])));
  let receiptShare = $derived(
    detail && detail.emailsPerYear > 0
      ? Math.round((detail.receiptsPerYear / detail.emailsPerYear) * 100)
      : null,
  );
  const kindLabel: Record<string, string> = {
    charge: '',
    upgrade: 'upgrade',
    downgrade: 'downgrade',
    refund: 'refund',
    trial_ending: 'trial ending',
    payment_failed: 'payment failed',
    card_expiring: 'card expiring',
    price_change: 'price change',
    cancellation: 'cancellation',
  };
</script>

<!-- S06 — Service detail -->
<a class="back" href="/subscriptions"><ArrowLeft size={15} strokeWidth={2.75} />Subscriptions</a>

{#if problem}
  <p class="problem" role="alert">{problem}</p>
{:else if detail === null}
  <p class="problem">This service is no longer in the local data. A new scan may have regrouped it.</p>
{:else if detail}
  <header>
    <div class="avatar">{initials(detail.name)}</div>
    <div class="who">
      <div class="title">
        <h1>{detail.name}</h1>
        {#if detail.isCritical}<Marker kind="critical" />{/if}
        {#if rise !== null}
          <Marker kind="price" label={`price ↑ ${rise}% this year`} />
        {:else if detail.priceIncrease}
          <Marker kind="price" label="price ↑" />
        {/if}
      </div>
      <p>
        {[
          detail.senders[0],
          detail.cadence,
          detail.firstSeen ? `member since ${monthLabel(detail.firstSeen.slice(0, 7))}` : null,
        ]
          .filter(Boolean)
          .join(' · ')}
      </p>
    </div>
    <div class="cost">
      {#if detail.monthly}
        <div class="figure">{money(detail.monthly)}<span>/mo</span></div>
        <p>
          {detail.lastCharge ? `last charged ${shortDate(detail.lastCharge.at)} · ` : ''}{money(
            { ...detail.monthly, minorUnits: detail.monthly.minorUnits * 12 },
            { whole: true },
          )}/yr
        </p>
      {:else if detail.lastCharge}
        <div class="figure">{money(detail.lastCharge.amount)}</div>
        <p>last charged {shortDate(detail.lastCharge.at)} · irregular</p>
      {/if}
    </div>
  </header>

  <div class="body">
    <div class="main">
      <section class="card">
        <div class="card-head">
          <h2>Spend history</h2>
          {#if chart}<span>{chart.from.slice(0, 4)} → today</span>{/if}
        </div>
        {#if chart}
          <svg viewBox="0 0 640 120" preserveAspectRatio="none" aria-hidden="true">
            <path d={chart.area} class="area" />
            <path d={chart.line} class="line" vector-effect="non-scaling-stroke" />
          </svg>
          <div class="levels">
            {#each chart.levels as level, i (i)}<span>{money(level)}</span>{/each}
          </div>
        {:else}
          <p class="quiet">No charges yet.</p>
        {/if}
      </section>

      <div class="pair">
        <section class="card">
          <h2>Email volume</h2>
          <div class="bars">
            {#each detail.volume as v, i (v.month)}
              <div
                class="bar"
                class:now={i === detail.volume.length - 1}
                style:height={`${(v.messages / maxBar) * 100}%`}
                title={`${monthLabel(v.month)}: ${v.messages}`}
              ></div>
            {/each}
          </div>
          <p class="quiet">
            {count(detail.emailsPerYear)} emails/yr{receiptShare !== null
              ? ` · ${receiptShare}% receipts`
              : ''}
          </p>
        </section>
        <section class="card">
          <h2>Price changes</h2>
          {#if detail.priceChanges.length === 0}
            <p class="quiet">The price has not changed.</p>
          {:else}
            <ul class="changes">
              {#each [...detail.priceChanges].reverse() as c, i (c.at)}
                <li>
                  <span class="mut">{monthLabel(c.at.slice(0, 7))}</span>
                  <span class="mono" class:latest={i === 0}>{money(c.from)} → {money(c.to)}</span>
                </li>
              {/each}
            </ul>
          {/if}
        </section>
      </div>

      <section class="card">
        <h2>Receipts</h2>
        {#if detail.receipts.length === 0}
          <p class="quiet">No receipts.</p>
        {:else}
          <ul class="receipts">
            {#each detail.receipts as r (r.messageId)}
              <li>
                <span class="subject">{longDate(r.date)} — {r.subject ?? '(no subject)'}</span>
                <span class="amount">
                  {#if kindLabel[r.kind]}<em>{kindLabel[r.kind]}</em>{/if}
                  {r.amount ? money(r.amount) : ''}
                </span>
              </li>
            {/each}
          </ul>
        {/if}
      </section>
    </div>

    <aside>
      <section class="card">
        <h2>End it</h2>
        <!-- Unsubscribe arrives in M3, playbooks in M4, the agent in M7. -->
        <div class="actions">
          <Button size="medium" disabled>Cancel via playbook</Button>
          <Button size="medium" variant="outline" disabled>Unsubscribe from emails</Button>
          <Button size="medium" variant="outline" disabled>
            Cancel via agent <Marker kind="experimental" />
          </Button>
        </div>
        <p class="quiet">These actions arrive in a later version. The figures here are final.</p>
      </section>
      {#if detail.isCritical}
        <div class="critical">
          <ShieldAlert size={16} strokeWidth={2.75} />
          <span>
            <strong>Critical service.</strong> Losing {detail.name} can lock you out of other accounts
            or data. Every action on it asks you first.
          </span>
        </div>
      {/if}
    </aside>
  </div>
{/if}

<style>
  .back {
    display: inline-flex;
    align-items: center;
    gap: 0.375rem;
    font-size: 0.84rem;
    font-weight: 600;
    color: var(--mut);
    text-decoration: none;
  }

  .back:hover {
    color: var(--fg);
  }

  header {
    display: flex;
    align-items: center;
    gap: 1.125rem;
    margin-top: 1.125rem;
    flex-wrap: wrap;
  }

  .avatar {
    width: 4rem;
    height: 4rem;
    flex: none;
    border-radius: var(--pill);
    background: var(--sage);
    color: var(--onSage);
    display: flex;
    align-items: center;
    justify-content: center;
    font-family: var(--font-heading);
    font-weight: var(--font-heading-weight);
    font-size: 1.375rem;
  }

  .who {
    flex: 1;
    min-width: 14rem;
  }

  .title {
    display: flex;
    align-items: center;
    gap: 0.625rem;
    flex-wrap: wrap;
  }

  h1 {
    margin: 0;
    font-family: var(--font-heading);
    font-weight: var(--font-heading-weight);
    font-size: 1.875rem;
    color: var(--hd);
  }

  header p {
    margin: 0.1875rem 0 0;
    font-size: 0.84rem;
    color: var(--mut);
  }

  .cost {
    text-align: right;
  }

  .figure {
    font-family: var(--font-heading);
    font-weight: var(--font-heading-weight);
    font-size: 2.5rem;
    color: var(--hd);
  }

  .figure span {
    font-size: 1.0625rem;
    color: var(--mut);
  }

  .body {
    display: flex;
    gap: 1.25rem;
    margin-top: 1.5rem;
    align-items: flex-start;
    flex-wrap: wrap;
  }

  .main {
    flex: 1 1 30rem;
    display: flex;
    flex-direction: column;
    gap: 1.25rem;
  }

  aside {
    flex: 0 1 21.25rem;
    display: flex;
    flex-direction: column;
    gap: 0.75rem;
  }

  .card {
    background: var(--card);
    border: var(--stroke) solid var(--line);
    border-radius: var(--radius-lg);
    padding: 1.25rem 1.5rem;
    box-shadow: var(--shadow-sm);
  }

  .card-head {
    display: flex;
    justify-content: space-between;
    align-items: baseline;
  }

  .card-head span {
    font-size: 0.75rem;
    color: var(--mut);
  }

  h2 {
    margin: 0;
    font-family: var(--font-heading);
    font-weight: var(--font-heading-weight);
    font-size: 1rem;
    color: var(--hd);
  }

  svg {
    display: block;
    width: 100%;
    height: 7.5rem;
    margin-top: 0.75rem;
  }

  .line {
    fill: none;
    stroke: var(--acc);
    stroke-width: 3;
  }

  .area {
    fill: var(--accSoft);
    opacity: 0.6;
  }

  .levels {
    display: flex;
    justify-content: space-between;
    margin-top: 0.375rem;
    font-family: var(--font-mono);
    font-size: 0.72rem;
    color: var(--mut);
  }

  .pair {
    display: flex;
    gap: 1.25rem;
    flex-wrap: wrap;
  }

  .pair .card {
    flex: 1 1 14rem;
  }

  .bars {
    display: flex;
    align-items: flex-end;
    gap: 0.375rem;
    height: 5.125rem;
    margin-top: 0.875rem;
  }

  .bar {
    flex: 1;
    min-height: 0.125rem;
    background: var(--sage);
    border-radius: 0.375rem 0.375rem 0 0;
    opacity: 0.55;
  }

  .bar.now {
    opacity: 1;
  }

  .quiet {
    margin: 0.5rem 0 0;
    font-size: 0.75rem;
    line-height: 1.5;
    color: var(--mut);
  }

  ul {
    list-style: none;
    margin: 0.75rem 0 0;
    padding: 0;
  }

  .changes li {
    display: flex;
    justify-content: space-between;
    font-size: 0.84rem;
  }

  .changes li + li {
    margin-top: 0.5625rem;
  }

  .mut {
    color: var(--mut);
  }

  .mono {
    font-family: var(--font-mono);
  }

  .latest {
    font-weight: 700;
    color: var(--accDeep);
  }

  .receipts {
    font-family: var(--font-mono);
    font-size: 0.8125rem;
  }

  .receipts li {
    display: flex;
    justify-content: space-between;
    gap: 1rem;
    padding: 0.4375rem 0;
    border-bottom: var(--stroke) dashed var(--line);
  }

  .receipts li:last-child {
    border-bottom: none;
  }

  .subject {
    min-width: 0;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .amount {
    flex: none;
    display: flex;
    gap: 0.75rem;
  }

  .amount em {
    font-family: var(--font-body);
    font-style: normal;
    font-size: 0.72rem;
    font-weight: 700;
    color: var(--accDeep);
  }

  .actions {
    display: flex;
    flex-direction: column;
    gap: 0.5625rem;
    margin-top: 0.875rem;
  }

  .critical {
    display: flex;
    gap: 0.625rem;
    background: var(--sageSoft);
    border-radius: var(--radius-lg);
    padding: 1.125rem 1.25rem;
    font-size: 0.8125rem;
    line-height: 1.55;
    color: var(--sageDeep);
  }

  .critical :global(svg) {
    flex: none;
    margin-top: 0.1875rem;
  }

  .problem {
    color: var(--bad);
  }
</style>
