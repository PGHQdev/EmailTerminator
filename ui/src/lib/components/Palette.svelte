<script lang="ts">
  import { goto } from '$app/navigation';
  import { Command, Dialog } from 'bits-ui';
  import {
    ArrowRight,
    Clock,
    CreditCard,
    House,
    Mail,
    MailX,
    MessageSquareWarning,
    Moon,
    Plus,
    RefreshCw,
    Search,
    Sun,
    SunMoon,
  } from 'lucide-svelte';
  import { appearance } from '$lib/appearance.svelte';
  import {
    commands,
    type Newsletter,
    type SourceSummary,
    type Subscription,
  } from '$lib/bindings';
  import { frequency, money, shortDate } from '$lib/format';
  import { sweep } from '$lib/sweep.svelte';

  // S18 — search across services, senders, screens and actions. The index is
  // the lists themselves, loaded when the palette opens (PLAN.md 2.1).
  let { open = $bindable(false) }: { open?: boolean } = $props();

  let search = $state('');
  let services = $state<Subscription[]>([]);
  let senders = $state<Newsletter[]>([]);
  let sources = $state<SourceSummary[]>([]);

  const mac = typeof navigator !== 'undefined' && /Mac/.test(navigator.userAgent);

  $effect(() => {
    if (!open) return;
    search = '';
    commands.subscriptions().then((r) => r.status === 'ok' && (services = r.data));
    commands.newsletters().then((r) => r.status === 'ok' && (senders = r.data));
    commands.listSources().then((r) => r.status === 'ok' && (sources = r.data));
  });

  function run(action: () => unknown) {
    open = false;
    action();
  }

  const screens = [
    { label: 'Home', href: '/home', icon: House },
    { label: 'Subscriptions', href: '/subscriptions', icon: CreditCard },
    { label: 'Newsletters', href: '/newsletters', icon: Mail },
    { label: 'Activity log', href: '/activity', icon: Clock },
    { label: 'Settings', href: '/settings', icon: Sun },
    { label: 'Report an issue', href: '/report', icon: MessageSquareWarning },
  ];
  const themes = [
    { label: 'Light appearance', value: 'light', icon: Sun },
    { label: 'Dark appearance', value: 'dark', icon: Moon },
    { label: 'System appearance', value: 'system', icon: SunMoon },
  ] as const;
</script>

<Dialog.Root bind:open>
  <Dialog.Portal>
    <Dialog.Overlay class="palette-scrim" />
    <Dialog.Content class="palette" aria-label="Command palette">
      <Command.Root loop>
        <div class="field">
          <Search size={17} strokeWidth={2.75} />
          <Command.Input bind:value={search} placeholder="Search services, senders, screens…" />
          <kbd>{mac ? '⌘K' : 'Ctrl K'}</kbd>
        </div>
        <Command.List class="list">
          <Command.Viewport>
            <Command.Empty class="empty">Nothing matches “{search}”.</Command.Empty>

            {#if services.length > 0}
              <Command.Group>
                <Command.GroupHeading class="heading">Services</Command.GroupHeading>
                <Command.GroupItems>
                  {#each services as s (s.id)}
                    <Command.Item
                      value={`service-${s.id}`}
                      keywords={[s.name]}
                      onSelect={() => run(() => goto(`/service?id=${s.id}`))}
                    >
                      <span class="tile acc"><ArrowRight size={15} strokeWidth={2.75} /></span>
                      <span class="text">
                        <strong>Open {s.name}</strong>
                        <small>
                          {s.monthly ? `${money(s.monthly)}/mo` : 'irregular'}
                          · last charged {shortDate(s.lastChargeAt)}
                          {s.priceIncrease ? '· price raised' : ''}
                        </small>
                      </span>
                    </Command.Item>
                  {/each}
                </Command.GroupItems>
              </Command.Group>
            {/if}

            {#if senders.length > 0}
              <Command.Group>
                <Command.GroupHeading class="heading">Newsletters</Command.GroupHeading>
                <Command.GroupItems>
                  {#each senders as n (n.id)}
                    <Command.Item
                      value={`sender-${n.id}`}
                      keywords={[n.name, n.address]}
                      onSelect={() => run(() => goto(`/newsletters?q=${encodeURIComponent(n.name)}`))}
                    >
                      <span class="tile"><Mail size={15} strokeWidth={2.75} /></span>
                      <span class="text">
                        <strong>{n.name}</strong>
                        <small>{n.address} · {frequency(n.perWeek, n.received)}</small>
                      </span>
                      {#if n.oneClick}<span class="hint ok">one-click</span>{/if}
                    </Command.Item>
                  {/each}
                  <!-- The action on a sender, offered once the search names it. -->
                  {#if search.trim()}
                    {#each senders.filter((n) => n.unsubscribedAt === null) as n (n.id)}
                      <Command.Item
                        value={`unsubscribe-${n.id}`}
                        keywords={[`unsubscribe ${n.name}`, n.address]}
                        onSelect={() => run(() => sweep.begin([{ kind: 'sender', id: n.id }]))}
                      >
                        <span class="tile acc"><MailX size={15} strokeWidth={2.75} /></span>
                        <span class="text">
                          <strong>Unsubscribe from {n.name}</strong>
                          <small>{n.oneClick ? 'one-click' : 'no one-click'}</small>
                        </span>
                      </Command.Item>
                    {/each}
                  {/if}
                </Command.GroupItems>
              </Command.Group>
            {/if}

            <Command.Group>
              <Command.GroupHeading class="heading">Navigate</Command.GroupHeading>
              <Command.GroupItems>
                {#if search.trim()}
                  <Command.Item
                    value="filter-subscriptions"
                    forceMount
                    onSelect={() =>
                      run(() => goto(`/subscriptions?q=${encodeURIComponent(search.trim())}`))}
                  >
                    <span class="tile"><CreditCard size={15} strokeWidth={2.75} /></span>
                    <span class="text"><strong>Subscriptions → filtered to “{search.trim()}”</strong></span>
                  </Command.Item>
                {/if}
                {#each screens as screen (screen.href)}
                  <Command.Item
                    value={`screen-${screen.href}`}
                    keywords={[screen.label]}
                    onSelect={() => run(() => goto(screen.href))}
                  >
                    <span class="tile"><screen.icon size={15} strokeWidth={2.75} /></span>
                    <span class="text"><strong>{screen.label}</strong></span>
                  </Command.Item>
                {/each}
              </Command.GroupItems>
            </Command.Group>

            <Command.Group>
              <Command.GroupHeading class="heading">Actions</Command.GroupHeading>
              <Command.GroupItems>
                {#each sources as source (source.id)}
                  <Command.Item
                    value={`scan-${source.id}`}
                    keywords={['scan again', 'sync', source.label]}
                    onSelect={() => run(() => goto(`/scan?source=${source.id}`))}
                  >
                    <span class="tile"><RefreshCw size={15} strokeWidth={2.75} /></span>
                    <span class="text"><strong>Scan again: {source.label}</strong></span>
                  </Command.Item>
                {/each}
                <Command.Item
                  value="add-source"
                  keywords={['add a source', 'connect', 'imap', 'mailbox']}
                  onSelect={() => run(() => goto('/welcome'))}
                >
                  <span class="tile"><Plus size={15} strokeWidth={2.75} /></span>
                  <span class="text"><strong>Add a source</strong></span>
                </Command.Item>
                {#each themes as theme (theme.value)}
                  <Command.Item
                    value={`theme-${theme.value}`}
                    keywords={[theme.label, 'theme', 'appearance']}
                    onSelect={() => run(() => appearance.set(theme.value))}
                  >
                    <span class="tile"><theme.icon size={15} strokeWidth={2.75} /></span>
                    <span class="text"><strong>{theme.label}</strong></span>
                  </Command.Item>
                {/each}
              </Command.GroupItems>
            </Command.Group>
          </Command.Viewport>
        </Command.List>
        <div class="keys">
          <span>↑↓ navigate</span><span>↵ run</span><span class="end">esc close</span>
        </div>
      </Command.Root>
    </Dialog.Content>
  </Dialog.Portal>
</Dialog.Root>

<style>
  :global(.palette-scrim) {
    position: fixed;
    inset: 0;
    background: var(--scrim);
    z-index: 10;
  }

  :global(.palette) {
    position: fixed;
    left: 50%;
    top: 7.5rem;
    transform: translateX(-50%);
    width: min(38.75rem, calc(100vw - 2rem));
    background: var(--card);
    border-radius: var(--radius-lg);
    box-shadow: var(--shadow-lg);
    overflow: hidden;
    z-index: 11;
    font-family: var(--font-body);
    color: var(--fg);
  }

  .field {
    display: flex;
    align-items: center;
    gap: 0.75rem;
    padding: 1.125rem 1.5rem;
    border-bottom: var(--stroke) solid var(--line);
    color: var(--mut);
  }

  .field :global(input) {
    flex: 1;
    border: none;
    outline: none;
    background: transparent;
    font: inherit;
    font-size: 1.03rem;
    color: var(--fg);
  }

  .field :global(input::placeholder) {
    color: var(--mut);
  }

  kbd {
    font-family: var(--font-mono);
    font-size: 0.6875rem;
    font-weight: 700;
    color: var(--mut);
    background: var(--card2);
    padding: 0.25rem 0.5625rem;
    border-radius: var(--radius-sm);
  }

  :global(.list) {
    max-height: min(26rem, 60vh);
    overflow-y: auto;
    padding: 0.625rem 0.625rem 0.375rem;
  }

  :global(.heading) {
    font-size: 0.656rem;
    font-weight: 800;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--mut);
    padding: 0.625rem 1rem 0.375rem;
  }

  :global(.empty) {
    padding: 1rem;
    font-size: 0.875rem;
    color: var(--mut);
  }

  :global([data-command-item]) {
    display: flex;
    align-items: center;
    gap: 0.8125rem;
    padding: 0.6875rem 1rem;
    border-radius: 0.875rem;
    cursor: pointer;
  }

  :global([data-command-item]:hover) {
    background: var(--card2);
  }

  :global([data-command-item][data-selected]) {
    background: var(--sageSoft);
  }

  .tile {
    width: 2rem;
    height: 2rem;
    flex: none;
    border-radius: 0.625rem;
    background: var(--card2);
    color: var(--mut);
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .tile.acc {
    background: var(--acc);
    color: var(--onAcc);
  }

  .text {
    flex: 1;
    min-width: 0;
  }

  .text strong {
    display: block;
    font-size: 0.906rem;
    font-weight: 600;
    color: var(--fg);
  }

  .text small {
    display: block;
    font-size: 0.75rem;
    color: var(--mut);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .hint {
    font-size: 0.75rem;
    font-weight: 700;
  }

  .hint.ok {
    color: var(--ok);
  }

  .keys {
    display: flex;
    gap: 1rem;
    padding: 0.75rem 1.5rem;
    border-top: var(--stroke) solid var(--line);
    font-family: var(--font-mono);
    font-size: 0.72rem;
    color: var(--mut);
  }

  .end {
    margin-left: auto;
  }
</style>
