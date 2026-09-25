import { describe, expect, it } from 'vitest';
import { ago, bytes, frequency, initials, money, monthLabel, rate, sum } from './format';

describe('money', () => {
  it('reads minor units by the currency', () => {
    expect(money({ minorUnits: 1700, currency: 'USD' })).toMatch(/17\.00/);
    expect(money({ minorUnits: 1200, currency: 'JPY' })).toMatch(/1,200/);
    expect(money({ minorUnits: 14250, currency: 'USD' }, { whole: true })).toMatch(/\$143$/);
  });

  it('sums one currency only', () => {
    const total = sum(
      [
        { minorUnits: 100, currency: 'USD' },
        { minorUnits: 999, currency: 'EUR' },
        { minorUnits: 250, currency: 'USD' },
      ],
      'USD',
    );
    expect(total).toEqual({ minorUnits: 350, currency: 'USD' });
  });
});

describe('frequency', () => {
  it('names the mockup rates', () => {
    expect(frequency(14, 728)).toBe('2×/day');
    expect(frequency(7, 364)).toBe('daily');
    expect(frequency(4, 208)).toBe('4×/wk');
    expect(frequency(1, 52)).toBe('weekly');
    expect(frequency(0.23, 12)).toBe('monthly');
    expect(frequency(0.05, 3)).toBe('rarely');
    expect(frequency(1, 1)).toBe('once');
  });

  it('turns a year of mail into a rate', () => {
    expect(rate(728)).toBe('14/wk');
    expect(rate(36)).toBe('3/mo');
    expect(rate(5)).toBe('5/yr');
  });
});

describe('labels', () => {
  it('makes two-letter initials', () => {
    expect(initials('NYTimes')).toBe('NY');
    expect(initials('The Dispatch')).toBe('TD');
    expect(initials('iCloud+')).toBe('IC');
  });

  it('names a month', () => {
    expect(monthLabel('2026-05')).toMatch(/May.*2026/);
  });

  it('says how long ago', () => {
    const now = Date.parse('2026-09-26T12:00:00Z');
    expect(ago('2026-09-26T11:58:00Z', now)).toMatch(/2 min/);
    expect(ago(null, now)).toBe('never');
  });

  it('sizes files', () => {
    expect(bytes(3.1 * 1024 ** 3)).toBe('3.1 GB');
    expect(bytes(512)).toBe('512 B');
    expect(bytes(null)).toBe('');
  });
});
