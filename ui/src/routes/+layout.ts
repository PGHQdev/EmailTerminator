export const prerender = true;
export const ssr = false;

export async function load() {
  // The design preview in a plain browser (src/lib/dev/mock.ts).
  if (import.meta.env.DEV && !('__TAURI_INTERNALS__' in window)) await import('$lib/dev/mock');
}
