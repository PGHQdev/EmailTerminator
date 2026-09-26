<script lang="ts">
  import { goto } from '$app/navigation';
  import { page } from '$app/state';
  import { openUrl } from '@tauri-apps/plugin-opener';
  import { ArrowLeft, Check } from 'lucide-svelte';
  import { commands, type PlaybookView } from '$lib/bindings';
  import Button from '$lib/components/Button.svelte';
  import { ago, longDate, money } from '$lib/format';
  import { paidThrough } from '$lib/history';
  import { sweep } from '$lib/sweep.svelte';

  // S08 — a playbook the user follows by hand. The app sends nothing; when
  // every step is done the user records the cancellation (PLAN.md M4).
  let view = $state<PlaybookView | null | undefined>(undefined);
  let problem = $state<string | null>(null);
  let allowed = $state(false);
  let done = $state<boolean[]>([]);
  let recording = $state(false);

  let id = $derived(Number(page.url.searchParams.get('id')));

  $effect(() => {
    commands.playbook(id).then(async (r) => {
      if (r.status === 'error') {
        problem = r.error;
        return;
      }
      view = r.data;
      done = r.data?.steps.map(() => false) ?? [];
      // A critical service asks S10 before its steps show.
      if (r.data?.service.isCritical && !(await sweep.confirm(r.data.service.name, 'cancel'))) {
        goto(`/service?id=${id}`);
        return;
      }
      allowed = true;
    });
  });

  let finished = $derived(done.filter(Boolean).length);
  let current = $derived(done.indexOf(false));
  let canceled = $derived(view?.service.status === 'canceled');
  let through = $derived(
    view ? paidThrough(view.service.lastCharge?.at ?? null, view.service.cadence) : null,
  );

  function host(url: string): string {
    return url.replace(/^https:\/\/(www\.)?/, '').replace(/[?#].*$/, '').replace(/\/$/, '');
  }

  async function record() {
    if (!view) return;
    recording = true;
    const r = await commands.finishPlaybook(view.service.id);
    recording = false;
    if (r.status === 'error') problem = r.error;
    else view.service.status = 'canceled';
  }
</script>

<!-- S08 — Playbook -->
{#if view}
  <a class="back" href="/service?id={view.service.id}">
    <ArrowLeft size={15} strokeWidth={2.75} />{view.service.name}
  </a>
{/if}

{#if problem}
  <p class="problem" role="alert">{problem}</p>
{:else if view === null}
  <p class="problem">No playbook for this service yet.</p>
{:else if view && allowed}
  <div class="layout">
    <div class="main">
      <h1>Cancel {view.service.name} — playbook</h1>
      <p class="sub">
        {[
          `${view.steps.length} ${view.steps.length === 1 ? 'step' : 'steps'}`,
          view.service.playbook?.minutes ? `about ${view.service.playbook.minutes} minutes` : null,
          view.service.playbook ? `checked ${ago(view.service.playbook.checked)}` : null,
        ]
          .filter(Boolean)
          .join(' · ')}
      </p>

      <ol>
        {#each view.steps as step, i (i)}
          <li class:done={done[i]} class:current={i === current && !canceled}>
            {#if done[i]}
              <span class="badge"><Check size={16} strokeWidth={2.75} /></span>
            {:else}
              <span class="badge">{i + 1}</span>
            {/if}
            <div class="step">
              <div class="text">{step.text}</div>
              {#if step.link}
                {@const link = step.link}
                <button type="button" class="link" onclick={() => openUrl(link)}>{host(link)}</button>
              {/if}
              {#if i === current && !canceled}
                <button type="button" class="mark" onclick={() => (done[i] = true)}>
                  Mark step done
                </button>
              {/if}
            </div>
          </li>
        {/each}
      </ol>

      <p class="foot">
        Steps from <button type="button" class="link" onclick={() => openUrl(view!.source)}>
          {host(view.source).split('/')[0]}
        </button>. Wrong or out of date?
        <button type="button" class="link" onclick={() => openUrl(view!.improve)}>
          Improve this playbook
        </button> — opens the community repo.
      </p>
    </div>

    <aside>
      <section class="card">
        <h2>Progress</h2>
        <div class="meter">
          <div class="track">
            <div class="fill" style:width="{(finished / view.steps.length) * 100}%"></div>
          </div>
          <span>{finished}/{view.steps.length}</span>
        </div>
        {#if canceled}
          <p>Recorded as cancelled. The entry is in Activity.</p>
          <Button variant="outline" size="medium" onclick={() => goto('/activity')}>
            Open Activity
          </Button>
        {:else if current === -1}
          <p>Did the cancellation go through? Record it, and {view.service.name} shows as cancelled.</p>
          <Button variant="dark" size="medium" disabled={recording} onclick={record}>
            I cancelled it
          </Button>
        {:else}
          <p>Do each step on {view.service.name}'s site, then mark it here. The app sends nothing.</p>
        {/if}
      </section>
      {#if view.service.monthly}
        <div class="saving">
          <strong>You're saving {money(view.service.monthly)}/mo.</strong>
          {#if through}Your current period runs through {longDate(through)}.{/if}
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

  .layout {
    display: flex;
    gap: 3.25rem;
    margin-top: 1.25rem;
    max-width: 66.25rem;
  }

  .main {
    flex: 1;
    min-width: 0;
    max-width: 40rem;
  }

  h1 {
    margin: 0;
    font-family: var(--font-heading);
    font-weight: var(--font-heading-weight);
    font-size: 1.875rem;
    color: var(--hd);
  }

  .sub {
    margin: 0.375rem 0 0;
    font-size: 0.875rem;
    color: var(--mut);
  }

  ol {
    list-style: none;
    margin: 1.625rem 0 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 0.75rem;
  }

  li {
    display: flex;
    gap: 1rem;
    background: var(--card);
    border: var(--stroke) solid var(--line);
    border-radius: var(--radius-lg);
    padding: 1.125rem 1.375rem;
  }

  li.done {
    opacity: 0.62;
  }

  li.current {
    border: var(--stroke-strong) solid var(--sage);
    box-shadow: var(--shadow-md);
  }

  .badge {
    width: 1.875rem;
    height: 1.875rem;
    flex: none;
    box-sizing: border-box;
    border-radius: var(--pill);
    border: var(--stroke-strong) solid var(--line);
    color: var(--mut);
    display: flex;
    align-items: center;
    justify-content: center;
    font-weight: 800;
    font-size: 0.875rem;
  }

  .current .badge {
    border-color: var(--sage);
    color: var(--sageDeep);
  }

  .done .badge {
    border: none;
    background: var(--sage);
    color: var(--onSage);
  }

  .step {
    flex: 1;
    min-width: 0;
  }

  .text {
    font-weight: 700;
    color: var(--hd);
    line-height: 1.45;
  }

  .done .text {
    text-decoration: line-through;
  }

  .link {
    padding: 0;
    border: none;
    background: none;
    font: inherit;
    font-weight: 700;
    color: var(--accDeep);
    cursor: pointer;
    overflow-wrap: anywhere;
    text-align: left;
  }

  .link:hover {
    opacity: 0.8;
  }

  .step .link {
    display: block;
    margin-top: 0.1875rem;
    font-size: 0.844rem;
  }

  .mark {
    margin-top: 0.75rem;
    border: none;
    cursor: pointer;
    background: var(--sageDeep);
    color: var(--onSage);
    font-family: var(--font-body);
    font-size: 0.8125rem;
    font-weight: 700;
    padding: 0.5625rem 1.25rem;
    border-radius: var(--pill);
  }

  .mark:hover {
    filter: brightness(1.15);
  }

  .foot {
    margin: 1.25rem 0 0;
    font-size: 0.8125rem;
    color: var(--mut);
    line-height: 1.6;
  }

  aside {
    width: 20rem;
    flex: none;
    display: flex;
    flex-direction: column;
    gap: 0.75rem;
    padding-top: 4rem;
  }

  .card {
    background: var(--card);
    border: var(--stroke) solid var(--line);
    border-radius: var(--radius-lg);
    padding: 1.25rem 1.375rem;
    box-shadow: var(--shadow-sm);
    display: flex;
    flex-direction: column;
    gap: 0.75rem;
  }

  h2 {
    margin: 0;
    font-size: 0.75rem;
    font-weight: 700;
    letter-spacing: 0.07em;
    text-transform: uppercase;
    color: var(--mut);
  }

  .meter {
    display: flex;
    align-items: center;
    gap: 0.75rem;
  }

  .track {
    flex: 1;
    height: 0.5rem;
    border-radius: var(--pill);
    background: var(--card2);
    overflow: hidden;
  }

  .fill {
    height: 100%;
    background: var(--sage);
    border-radius: var(--pill);
  }

  .meter span {
    font-family: var(--font-mono);
    font-size: 0.8125rem;
    font-weight: 700;
  }

  .card p {
    margin: 0;
    font-size: 0.8125rem;
    line-height: 1.5;
    color: var(--mut);
  }

  .saving {
    background: var(--accSoft);
    border-radius: var(--radius-lg);
    padding: 1.125rem 1.25rem;
    font-size: 0.8125rem;
    line-height: 1.55;
    color: var(--accDeep);
  }

  .problem {
    color: var(--bad);
  }
</style>
