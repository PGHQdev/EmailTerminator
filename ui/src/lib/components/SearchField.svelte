<script lang="ts">
  import { Search } from 'lucide-svelte';

  let { value = $bindable(''), placeholder }: { value?: string; placeholder: string } = $props();
</script>

<label>
  <Search size={14} strokeWidth={2.75} />
  <input type="search" bind:value {placeholder} aria-label={placeholder} />
</label>

<style>
  label {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    width: 13.75rem;
    box-sizing: border-box;
    background: var(--card);
    border: var(--stroke) solid var(--line);
    border-radius: var(--pill);
    padding: 0.5625rem 1rem;
    color: var(--mut);
  }

  label:focus-within {
    border-color: var(--sage);
  }

  input {
    flex: 1;
    min-width: 0;
    border: none;
    outline: none;
    background: transparent;
    font-family: var(--font-body);
    font-size: 0.84rem;
    color: var(--fg);
  }

  input::placeholder {
    color: var(--mut);
  }
</style>
