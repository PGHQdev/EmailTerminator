<script lang="ts">
  import '$lib/styles/tokens.css';
  import '$lib/styles/theme.css';
  import { appearance } from '$lib/appearance.svelte';
  import { resolveTheme } from '$lib/theme';

  let { children } = $props();

  $effect(() => {
    appearance.load();
  });

  $effect(() => {
    const setting = appearance.value;
    const query = window.matchMedia('(prefers-color-scheme: dark)');
    const apply = () => {
      document.documentElement.dataset.theme = resolveTheme(setting, query.matches);
    };
    apply();
    query.addEventListener('change', apply);
    return () => query.removeEventListener('change', apply);
  });
</script>

{@render children()}
