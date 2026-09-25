<script lang="ts">
  // S16's markers: `critical` on flagged services everywhere they appear,
  // `experimental` on every agentic surface, and the price-rise flag.
  let { kind, label }: { kind: 'critical' | 'experimental' | 'price'; label?: string } = $props();
</script>

<span class="marker {kind}">{label ?? kind}</span>

<style>
  .marker {
    font-size: 0.656rem;
    font-weight: 800;
    padding: 0.125rem 0.5625rem;
    border-radius: var(--pill);
    white-space: nowrap;
    color: var(--accDeep);
    background: var(--accSoft);
  }

  .critical {
    color: var(--sageDeep);
    background: var(--sageSoft);
  }

  .experimental {
    font-size: 0.625rem;
    letter-spacing: 0.05em;
    text-transform: uppercase;
  }
</style>
