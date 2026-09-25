<script lang="ts">
  import '$lib/styles/tokens.css';
  import '$lib/styles/theme.css';
  import { resolveTheme } from '$lib/theme';

  let { children } = $props();

  // The appearance setting arrives with S17 in M2; until then, follow the OS.
  $effect(() => {
    const query = window.matchMedia('(prefers-color-scheme: dark)');
    const apply = () => {
      document.documentElement.dataset.theme = resolveTheme('system', query.matches);
    };
    apply();
    query.addEventListener('change', apply);
    return () => query.removeEventListener('change', apply);
  });
</script>

{@render children()}
