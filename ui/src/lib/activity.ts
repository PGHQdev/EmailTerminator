import type { Entry } from '$lib/bindings';

/** S14's action column. An unsubscribe that sent a POST was one-click. */
export function actionLabel(entry: Entry): string {
  switch (entry.kind) {
    case 'unsubscribe':
      return entry.request?.startsWith('POST ') ? 'One-click unsubscribe' : 'Unsubscribe';
    case 'playbook':
      return 'Playbook cancellation';
    case 'agent':
      return 'Agent cancellation';
    case 'sync':
      return 'Mailbox sync';
  }
}

/** S14's outcome column: a sync that worked shows what it found. */
export function outcomeLabel(entry: Entry): string {
  if (entry.outcome === 'succeeded') return entry.kind === 'sync' ? entry.detail : 'succeeded';
  const word = entry.outcome === 'needsYou' ? 'needs you' : 'failed';
  return entry.detail ? `${word} — ${entry.detail}` : word;
}

/** The tone of a row's dot and outcome: done, needs attention, or neutral. */
export function tone(entry: Entry): 'ok' | 'bad' | 'mut' {
  if (entry.outcome !== 'succeeded') return 'bad';
  return entry.kind === 'sync' ? 'mut' : 'ok';
}

export type Filter = 'all' | 'unsubscribes' | 'cancellations' | 'failures' | 'syncs';

export const filters: Record<Filter, { label: string; keep: (e: Entry) => boolean }> = {
  all: { label: 'All', keep: () => true },
  unsubscribes: { label: 'Unsubscribes', keep: (e) => e.kind === 'unsubscribe' },
  cancellations: {
    label: 'Cancellations',
    keep: (e) => e.kind === 'playbook' || e.kind === 'agent',
  },
  failures: { label: 'Failures', keep: (e) => e.outcome !== 'succeeded' },
  syncs: { label: 'Syncs', keep: (e) => e.kind === 'sync' },
};
