<script lang="ts">
  let { size = 'large' }: { size?: 'large' | 'small' } = $props();
</script>

<div class="brand {size}">
  <img src="/logo.svg" alt="" />
  <span>EmailTerminator</span>
</div>

<style>
  .brand {
    display: flex;
    align-items: center;
    gap: var(--space-2);
  }

  img {
    width: 2.75rem;
    height: 2.75rem;
  }

  span {
    font-family: var(--font-heading);
    font-weight: var(--font-heading-weight);
    font-size: 1.625rem;
    color: var(--hd);
  }

  .small img {
    width: 2rem;
    height: 2rem;
  }

  .small span {
    font-size: 1.0625rem;
  }
</style>
