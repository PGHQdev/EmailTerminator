<script lang="ts">
  import { page } from '$app/state';

  // The Settings header S12 and S17 share. Intelligence (S13) arrives with M6.
  const on = (href: string) => page.url.pathname === href;
</script>

<header>
  <h1>Settings</h1>
  <nav class="tabs">
    <a href="/settings/sources" class:on={on('/settings/sources')}>Sources</a>
    <span class="off" aria-disabled="true" title="Arrives with intelligence">Intelligence</span>
    <a href="/settings" class:on={on('/settings')}>General</a>
  </nav>
</header>

<style>
  header {
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

  .tabs {
    display: flex;
    gap: 0.25rem;
    background: var(--card2);
    border-radius: var(--pill);
    padding: 0.25rem;
  }

  a,
  span {
    padding: 0.4375rem 1.125rem;
    border-radius: var(--pill);
    font-size: 0.8125rem;
    font-weight: 600;
    color: var(--mut);
    text-decoration: none;
  }

  a:hover {
    color: var(--fg);
  }

  .off {
    opacity: 0.45;
  }

  a.on {
    background: var(--card);
    color: var(--hd);
    font-weight: 700;
    box-shadow: var(--shadow-sm);
  }
</style>
