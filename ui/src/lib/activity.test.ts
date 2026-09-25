import { describe, expect, it } from 'vitest';
import type { Entry } from '$lib/bindings';
import { actionLabel, filters, outcomeLabel, tone } from './activity';

const entry = (over: Partial<Entry>): Entry => ({
  id: 1,
  kind: 'unsubscribe',
  target: 'The Dispatch',
  at: '2026-09-26T14:21:00Z',
  outcome: 'succeeded',
  detail: 'the sender answered 200',
  request: 'POST https://dispatch.test/u',
  evidence: null,
  ...over,
});

describe('activity rows', () => {
  it('names a one-click unsubscribe by the request it sent', () => {
    expect(actionLabel(entry({}))).toBe('One-click unsubscribe');
    expect(actionLabel(entry({ request: null, outcome: 'needsYou' }))).toBe('Unsubscribe');
    expect(actionLabel(entry({ kind: 'sync', request: null }))).toBe('Mailbox sync');
  });

  it('says why an action did not work', () => {
    expect(outcomeLabel(entry({}))).toBe('succeeded');
    expect(outcomeLabel(entry({ outcome: 'failed', detail: 'the sender answered 404' }))).toBe(
      'failed — the sender answered 404',
    );
    expect(outcomeLabel(entry({ kind: 'sync', detail: '312 new messages' }))).toBe(
      '312 new messages',
    );
    expect(tone(entry({ kind: 'sync' }))).toBe('mut');
    expect(tone(entry({ outcome: 'needsYou' }))).toBe('bad');
  });

  it('filters failures across kinds', () => {
    const rows = [entry({}), entry({ outcome: 'failed' }), entry({ kind: 'sync', outcome: 'failed' })];
    expect(rows.filter(filters.failures.keep)).toHaveLength(2);
    expect(rows.filter(filters.syncs.keep)).toHaveLength(1);
    expect(rows.filter(filters.cancellations.keep)).toHaveLength(0);
  });
});
