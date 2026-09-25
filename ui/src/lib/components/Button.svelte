<script lang="ts">
  import type { Snippet } from 'svelte';
  import type { HTMLButtonAttributes } from 'svelte/elements';

  // The mockups' pill buttons: `primary` on the accent, `outline` on the
  // line colour, `dark` in the text colour, `danger` for erasing.
  let {
    variant = 'primary',
    size = 'large',
    children,
    ...rest
  }: HTMLButtonAttributes & {
    variant?: 'primary' | 'outline' | 'dark' | 'danger';
    size?: 'large' | 'medium' | 'small';
    children: Snippet;
  } = $props();
</script>

<button type="button" class="{variant} {size}" {...rest}>{@render children()}</button>

<style>
  button {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 0.5625rem;
    cursor: pointer;
    font-family: var(--font-body);
    font-weight: 700;
    border-radius: var(--pill);
    border: var(--stroke-strong) solid transparent;
  }

  .large {
    font-size: 0.97rem;
    padding: 0.9375rem 1.5rem;
  }

  .medium {
    font-size: 0.906rem;
    padding: 0.75rem 1.5rem;
  }

  .small {
    font-size: 0.78rem;
    padding: 0.4375rem 1rem;
  }

  .primary {
    background: var(--acc);
    color: var(--onAcc);
    box-shadow: var(--shadow-sm);
  }

  .primary.large {
    box-shadow: var(--shadow-md);
  }

  .primary:hover {
    filter: brightness(1.08);
  }

  .primary:active {
    filter: brightness(0.95);
  }

  .outline {
    background: transparent;
    color: var(--fg);
    font-weight: 600;
    border-color: var(--line);
  }

  .outline:hover {
    background: var(--card2);
  }

  .outline:active {
    background: var(--line);
  }

  .dark {
    background: var(--fg);
    color: var(--bg);
  }

  .dark:hover {
    filter: brightness(1.2);
  }

  .danger {
    background: transparent;
    color: var(--accDeep);
    border-color: var(--accSoft);
  }

  .danger:hover {
    background: var(--accSoft);
  }

  button:disabled {
    opacity: 0.45;
    cursor: default;
    filter: none;
  }

  .outline:disabled:hover,
  .danger:disabled:hover {
    background: transparent;
  }
</style>
