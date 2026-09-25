<script lang="ts">
  import { page } from '$app/state';
  import { Clock, CreditCard, House, Mail, Sun, Target } from 'lucide-svelte';

  // Rail.dc.html: the navigation 12 screens share.
  const items = [
    { label: 'Home', href: '/home', icon: House, also: [] },
    { label: 'Subs', href: '/subscriptions', icon: CreditCard, also: ['/service'] },
    { label: 'News', href: '/newsletters', icon: Mail, also: [] },
    { label: 'Log', href: '/activity', icon: Clock, also: [] },
    { label: 'Set', href: '/settings', icon: Sun, also: ['/report'] },
  ];

  const active = (item: (typeof items)[number]) =>
    [item.href, ...item.also].some((p) => page.url.pathname.startsWith(p));
</script>

<nav>
  <img class="logo" src="/logo.svg" alt="EmailTerminator" />
  {#each items as item (item.href)}
    <a href={item.href} class:on={active(item)} aria-current={active(item) ? 'page' : undefined}>
      <item.icon size={18} strokeWidth={2.75} />
      <span>{item.label}</span>
    </a>
  {/each}
  <!-- The bulk sweep (S11) arrives with actions in M3. -->
  <button type="button" class="action" title="Clean up — arrives with bulk actions" disabled>
    <Target size={20} strokeWidth={2.75} />
  </button>
</nav>

<style>
  nav {
    position: fixed;
    inset: 0 auto 0 0;
    width: 5.375rem;
    box-sizing: border-box;
    background: var(--card);
    border-right: var(--stroke) solid var(--line);
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 0.375rem;
    padding: 1.25rem 0;
    z-index: 1;
  }

  .logo {
    width: 2.25rem;
    height: 2.25rem;
    margin-bottom: 1rem;
  }

  a {
    width: 3.875rem;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 0.25rem;
    padding: 0.625rem 0;
    border-radius: var(--radius-md);
    color: var(--mut);
    text-decoration: none;
  }

  a:hover {
    background: var(--card2);
  }

  a:active {
    background: var(--line);
  }

  a.on {
    background: var(--sageSoft);
    color: var(--sageDeep);
  }

  span {
    font-size: 0.625rem;
    font-weight: 800;
  }

  .action {
    margin-top: auto;
    width: 3.125rem;
    height: 3.125rem;
    border-radius: var(--pill);
    border: none;
    background: var(--acc);
    color: var(--onAcc);
    display: flex;
    align-items: center;
    justify-content: center;
    box-shadow: var(--shadow-md);
    cursor: pointer;
  }

  .action:disabled {
    opacity: 0.45;
    cursor: default;
  }
</style>
