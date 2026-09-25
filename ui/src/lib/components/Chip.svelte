<script lang="ts">
  // A filter pill on S04 and S05: the chosen one is filled.
  let {
    label,
    count,
    on,
    onclick,
  }: { label: string; count: number; on: boolean; onclick: () => void } = $props();
</script>

<button type="button" class:on aria-pressed={on} {onclick}>{label} · {count}</button>

<style>
  button {
    border: none;
    cursor: pointer;
    font-family: var(--font-body);
    font-size: 0.78rem;
    font-weight: 600;
    padding: 0.375rem 0.875rem;
    border-radius: var(--pill);
    background: var(--card2);
    color: var(--mut);
  }

  button:hover {
    color: var(--fg);
  }

  button:active {
    background: var(--line);
  }

  .on,
  .on:hover {
    background: var(--sageDeep);
    color: var(--sageSoft);
  }
</style>
