import { describe, expect, it } from 'vitest';
import type { MonthSpend } from '$lib/bindings';
import { paidThrough, priceRise, stepLine } from './history';

const usd = (minorUnits: number) => ({ minorUnits, currency: 'USD' });

describe('stepLine', () => {
  const spend: MonthSpend[] = [
    { month: '2026-01', amount: usd(1000) },
    { month: '2026-02', amount: usd(1000) },
    { month: '2026-03', amount: usd(1200) },
    { month: '2026-03', amount: { minorUnits: 500, currency: 'EUR' } },
  ];

  it('steps between prices and runs to this month', () => {
    const chart = stepLine(spend, 'USD', new Date('2026-05-15T00:00:00Z'))!;
    expect(chart.levels.map((l) => l.minorUnits)).toEqual([1000, 1200]);
    expect(chart.line.startsWith('M0.0')).toBe(true);
    expect(chart.line.endsWith('L640 22.0')).toBe(true);
    expect(chart.from).toBe('2026-01');
  });

  it('has nothing to draw in a currency without charges', () => {
    expect(stepLine(spend, 'GBP')).toBeNull();
  });
});

describe('priceRise', () => {
  const now = Date.parse('2026-09-01T00:00:00Z');
  it('adds up the rises of the last year', () => {
    const changes = [
      { at: '2024-01-01T00:00:00Z', from: usd(999), to: usd(1300) },
      { at: '2026-05-01T00:00:00Z', from: usd(1300), to: usd(1700) },
    ];
    expect(priceRise(changes, now)).toBe(31);
  });

  it('is null for a cut or an old rise', () => {
    expect(priceRise([{ at: '2026-05-01T00:00:00Z', from: usd(1700), to: usd(1300) }], now)).toBeNull();
    expect(priceRise([{ at: '2024-05-01T00:00:00Z', from: usd(1300), to: usd(1700) }], now)).toBeNull();
  });
});

describe('paidThrough', () => {
  const now = new Date('2026-09-26T00:00:00Z');

  it('adds one cadence to the last charge', () => {
    expect(paidThrough('2026-09-01T10:00:00Z', 'monthly', now)).toBe('2026-10-01T10:00:00.000Z');
    expect(paidThrough('2026-03-15T00:00:00Z', 'annual', now)).toBe('2027-03-15T00:00:00.000Z');
  });

  it('knows nothing without a cadence or once the period has passed', () => {
    expect(paidThrough('2026-09-01T10:00:00Z', 'irregular', now)).toBeNull();
    expect(paidThrough('2026-07-01T10:00:00Z', 'monthly', now)).toBeNull();
    expect(paidThrough(null, 'monthly', now)).toBeNull();
  });
});
