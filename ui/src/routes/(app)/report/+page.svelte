<script lang="ts">
  import { openUrl } from '@tauri-apps/plugin-opener';
  import { commands, type AppInfo } from '$lib/bindings';
  import Button from '$lib/components/Button.svelte';
  import { issue } from '$lib/issue';

  let description = $state('');
  let info = $state<AppInfo | null>(null);

  $effect(() => {
    commands.appInfo().then((i) => (info = i));
  });

  let draft = $derived(info ? issue(description, info) : null);
</script>

<!-- S15 — Report an issue -->
<h1>Report an issue</h1>
<p class="lede">Filed on GitHub, in the open — like the rest of the app.</p>

<div class="body">
  <div class="form">
    <label for="what">What happened?</label>
    <textarea id="what" bind:value={description} placeholder="What you did, what you expected, and what the app did instead."></textarea>
    <p class="promise">
      <strong>What we attach:</strong> the app version and your OS. <strong>Never</strong> message
      content, addresses, or anything from your mailbox. The text on the right is exactly what gets
      posted, and you can still edit it on GitHub before you submit.
    </p>
    <Button variant="dark" size="medium" disabled={!draft} onclick={() => draft && openUrl(draft.url)}>
      <svg width="16" height="16" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true"
        ><path
          d="M12 .5C5.65.5.5 5.65.5 12c0 5.1 3.29 9.4 7.86 10.94.58.1.79-.25.79-.55v-2.1c-3.2.7-3.87-1.36-3.87-1.36-.53-1.33-1.28-1.69-1.28-1.69-1.05-.71.08-.7.08-.7 1.16.08 1.77 1.19 1.77 1.19 1.03 1.77 2.7 1.26 3.36.96.1-.75.4-1.26.73-1.55-2.55-.29-5.23-1.28-5.23-5.68 0-1.26.45-2.28 1.19-3.09-.12-.29-.52-1.46.11-3.05 0 0 .97-.31 3.18 1.18a11 11 0 0 1 5.78 0c2.2-1.49 3.17-1.18 3.17-1.18.63 1.59.23 2.76.12 3.05.74.81 1.18 1.83 1.18 3.09 0 4.41-2.69 5.38-5.25 5.66.41.36.78 1.06.78 2.14v3.17c0 .3.2.66.8.55A11.5 11.5 0 0 0 23.5 12C23.5 5.65 18.35.5 12 .5z"
        /></svg
      >
      Open prefilled issue on GitHub
    </Button>
  </div>

  <section class="preview">
    <div class="head">
      <span>Exact issue preview</span>
      <span class="badge">nothing private</span>
    </div>
    {#if draft}
      <pre><strong>{draft.title}</strong>

{draft.body}</pre>
    {/if}
  </section>
</div>

<style>
  h1 {
    margin: 0;
    font-family: var(--font-heading);
    font-weight: var(--font-heading-weight);
    font-size: 1.875rem;
    color: var(--hd);
  }

  .lede {
    margin: 0.25rem 0 0;
    font-size: 0.875rem;
    color: var(--mut);
  }

  .body {
    display: flex;
    gap: 1.5rem;
    margin-top: 1.5rem;
    flex-wrap: wrap;
    align-items: flex-start;
  }

  .form {
    flex: 1 1 22rem;
    max-width: 32.5rem;
    display: flex;
    flex-direction: column;
    align-items: flex-start;
  }

  label {
    font-size: 0.75rem;
    font-weight: 700;
    letter-spacing: 0.07em;
    text-transform: uppercase;
    color: var(--mut);
  }

  textarea {
    align-self: stretch;
    box-sizing: border-box;
    margin-top: 0.625rem;
    height: 11.25rem;
    resize: vertical;
    border: var(--stroke-strong) solid var(--line);
    border-radius: var(--radius-lg);
    background: var(--card);
    padding: 1rem 1.25rem;
    font-family: var(--font-body);
    font-size: 0.906rem;
    line-height: 1.6;
    color: var(--fg);
    outline: none;
  }

  textarea:focus {
    border-color: var(--sage);
  }

  textarea::placeholder {
    color: var(--mut);
  }

  .promise {
    margin: 1.125rem 0;
    background: var(--sageSoft);
    border-radius: var(--radius-lg);
    padding: 1rem 1.25rem;
    font-size: 0.8125rem;
    line-height: 1.55;
    color: var(--sageDeep);
  }

  .preview {
    flex: 1 1 22rem;
    background: var(--card);
    border: var(--stroke) solid var(--line);
    border-radius: var(--radius-lg);
    box-shadow: var(--shadow-sm);
    overflow: hidden;
  }

  .head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 0.75rem 1.375rem;
    border-bottom: var(--stroke) solid var(--line);
    font-size: 0.75rem;
    font-weight: 700;
    letter-spacing: 0.07em;
    text-transform: uppercase;
    color: var(--mut);
  }

  .badge {
    font-size: 0.6875rem;
    font-weight: 800;
    letter-spacing: normal;
    text-transform: none;
    color: var(--sageDeep);
    background: var(--sageSoft);
    padding: 0.1875rem 0.625rem;
    border-radius: var(--pill);
  }

  pre {
    margin: 0;
    padding: 1.25rem 1.5rem;
    font-family: var(--font-mono);
    font-size: 0.78rem;
    line-height: 1.75;
    color: var(--fg);
    white-space: pre-wrap;
    word-break: break-word;
  }
</style>
