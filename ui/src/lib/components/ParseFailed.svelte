<script lang="ts">
  import { FileWarning } from 'lucide-svelte';
  import { count as n } from '$lib/format';

  let {
    message,
    onRetry,
    onKeep,
  }: { message: number; onRetry: () => void; onKeep: () => void } = $props();
</script>

<!-- S16, error · mbox parse failure -->
<div class="card" role="alert">
  <div class="title">
    <FileWarning size={18} strokeWidth={2.75} />
    Couldn't parse this mbox
  </div>
  <p>
    Stopped at message {n(message)} — the file may be truncated. We kept everything parsed so far.
  </p>
  <div class="actions">
    <button type="button" class="dark" onclick={onRetry}>Retry from {n(message)}</button>
    <button type="button" class="outline" onclick={onKeep}>Keep partial import</button>
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
    padding: var(--space-2) var(--space-4);
  }

  .dark {
    border: none;
    background: var(--fg);
    color: var(--bg);
  }

  .dark:hover {
    opacity: 0.85;
  }

  .outline {
    border: var(--stroke-strong) solid var(--line);
    background: none;
    color: var(--fg);
  }

  .outline:hover {
    border-color: var(--mut);
  }
</style>
