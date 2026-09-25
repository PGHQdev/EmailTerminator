<script lang="ts">
  import { openUrl } from '@tauri-apps/plugin-opener';
  import { CircleAlert } from 'lucide-svelte';

  let {
    host,
    serverSays,
    guide,
    onRetry,
  }: { host: string; serverSays: string; guide: string | null; onRetry: () => void } = $props();
</script>

<!-- S16, error · IMAP auth failure -->
<div class="card" role="alert">
  <div class="title">
    <CircleAlert size={18} strokeWidth={2.75} />
    IMAP sign-in failed
  </div>
  <p>
    {host} rejected the app password ({serverSays}). Regular account passwords won't work — generate
    an app password in your provider's settings.
  </p>
  <div class="actions">
    <button type="button" class="dark" onclick={onRetry}>Try another password</button>
    {#if guide}
      <button type="button" class="link" onclick={() => openUrl(guide)}>provider guide</button>
    {/if}
  </div>
</div>

<style>
  .card {
    background: var(--card);
    border: var(--stroke) solid var(--line);
    border-radius: var(--radius-lg);
    padding: var(--space-6);
    text-align: left;
  }

  .title {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    font-weight: 800;
    font-size: 0.875rem;
    color: var(--accDeep);
  }

  p {
    margin: var(--space-2) 0 0;
    font-size: 0.8125rem;
    line-height: 1.55;
    color: var(--mut);
  }

  .actions {
    display: flex;
    gap: var(--space-2);
    margin-top: var(--space-3);
  }

  button {
    font-family: var(--font-body);
    font-size: 0.78rem;
    font-weight: 700;
    cursor: pointer;
    border-radius: var(--pill);
  }

  .dark {
    border: none;
    background: var(--fg);
    color: var(--bg);
    padding: var(--space-2) var(--space-4);
  }

  .dark:hover {
    opacity: 0.85;
  }

  .link {
    border: none;
    background: none;
    color: var(--accDeep);
    padding: 0;
  }

  .link:hover {
    text-decoration: underline;
  }
</style>
