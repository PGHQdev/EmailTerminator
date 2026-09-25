import { describe, expect, it } from 'vitest';
import { FIELD, pack } from './bubbles';

describe('pack', () => {
  const costs = [2299, 1999, 1799, 1700, 1495, 1199, 999, 800, 1199, 1710];
  const placed = pack(costs, (c) => c);

  it('fits the mockup’s ten circles without overlap', () => {
    expect(placed).toHaveLength(10);
    for (const a of placed) {
      expect(a.x - a.d / 2).toBeGreaterThanOrEqual(0);
      expect(a.y + a.d / 2).toBeLessThanOrEqual(FIELD.height);
      for (const b of placed) {
        if (a === b) continue;
        expect(Math.hypot(a.x - b.x, a.y - b.y)).toBeGreaterThanOrEqual((a.d + b.d) / 2);
      }
    }
  });

  it('sizes by cost and puts the dearest first', () => {
    expect(placed[0].item).toBe(2299);
    expect(placed[0].d).toBe(195);
    expect(placed.at(-1)!.d).toBeLessThan(placed[0].d);
  });

  it('handles an empty list and zero costs', () => {
    expect(pack([], () => 0)).toEqual([]);
    expect(pack([0, 0], (c) => c).map((p) => p.d)).toEqual([76, 76]);
  });
});
