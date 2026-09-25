import { describe, expect, it } from 'vitest';
import { violations } from './check-tokens';

describe('violations', () => {
  it('accepts tokens and aliases', () => {
    expect(violations('padding: var(--space-4);\ncolor: var(--fg);\nfont-family: var(--font-body);')).toEqual([]);
  });

  it('reports raw hex, raw px, and foreign fonts by line', () => {
    const found = violations('color: #fff;\nmargin: 12px;\nfont-family: Arial;');
    expect(found.map((v) => v.line)).toEqual([1, 2, 3]);
  });
});
