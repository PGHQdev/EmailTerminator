<script lang="ts">
  import type { Snippet } from 'svelte';

  // S16's empty states: an icon in a soft circle, a title, one line, and an
  // optional action.
  let {
    icon,
    title,
    muted = false,
    children,
    action,
  }: {
    icon: Snippet;
    title: string;
    muted?: boolean;
    children: Snippet;
    action?: Snippet;
  } = $props();
</script>

<div class="empty">
  <div class="icon" class:muted>{@render icon()}</div>
  <h2>{title}</h2>
  <p>{@render children()}</p>
  {#if action}<div class="action">{@render action()}</div>{/if}
</div>

<style>
  .empty {
    max-width: 22rem;
    margin: 3rem auto;
    background: var(--card);
    border: var(--stroke) solid var(--line);
    border-radius: var(--radius-lg);
    padding: 1.625rem;
    text-align: center;
  }

  .icon {
    width: 3.625rem;
    height: 3.625rem;
    margin: 0 auto;
    border-radius: var(--pill);
    background: var(--sageSoft);
    color: var(--sageDeep);
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .icon.muted {
    background: var(--card2);
    color: var(--mut);
  }

  h2 {
    margin: 0.875rem 0 0;
    font-family: var(--font-heading);
    font-weight: var(--font-heading-weight);
    font-size: 1.0625rem;
    color: var(--hd);
  }

  p {
    margin: 0.375rem 0 0;
    font-size: 0.8125rem;
    line-height: 1.5;
    color: var(--mut);
  }

  .action {
    margin-top: 0.875rem;
  }
</style>
