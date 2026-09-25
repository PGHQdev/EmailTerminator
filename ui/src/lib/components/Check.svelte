<script lang="ts">
  import { Check as Tick } from 'lucide-svelte';

  // The row checkbox on S04 and S05. It sits inside a clickable row, so a
  // click on it must not open the row.
  let { checked, onchange, label }: { checked: boolean; onchange: () => void; label: string } =
    $props();
</script>

<button
  type="button"
  role="checkbox"
  aria-checked={checked}
  aria-label={label}
  class:checked
  onclick={(event) => {
    event.stopPropagation();
    onchange();
  }}
>
  {#if checked}<Tick size={12} strokeWidth={3.5} />{/if}
</button>

<style>
  button {
    width: 1.1875rem;
    height: 1.1875rem;
    padding: 0;
    border-radius: 0.375rem;
    border: var(--stroke-strong) solid var(--line);
    background: transparent;
    color: var(--onSage);
    display: flex;
    align-items: center;
    justify-content: center;
    cursor: pointer;
  }

  button:hover {
    border-color: var(--sage);
  }

  .checked {
    background: var(--sage);
    border-color: var(--sage);
  }
</style>
