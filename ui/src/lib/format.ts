import type { Amount, Cadence } from '$lib/bindings';

/** The UI copy is English, so dates and numbers are too. */
const LOCALE = 'en';

/** Fraction digits of a currency's minor unit: 2 for USD, 0 for JPY. */
function minorDigits(currency: string): number {
  try {
    return new Intl.NumberFormat('en', { style: 'currency', currency }).resolvedOptions()
      .maximumFractionDigits ?? 2;
  } catch {
    return 2;
  }
}

/** `$17.00`; `whole` rounds to `$142` for headline figures. */
export function money(amount: Amount, options: { whole?: boolean } = {}): string {
  const digits = minorDigits(amount.currency);
  const value = amount.minorUnits / 10 ** digits;
  const fraction = options.whole ? 0 : digits;
  try {
    return new Intl.NumberFormat(LOCALE, {
      style: 'currency',
      currency: amount.currency,
      minimumFractionDigits: fraction,
      maximumFractionDigits: fraction,
    }).format(value);
  } catch {
    return `${value.toFixed(fraction)} ${amount.currency}`;
  }
}

/** Sums amounts in one currency. */
export function sum(amounts: Amount[], currency: string): Amount {
  const minorUnits = amounts
    .filter((a) => a.currency === currency)
    .reduce((total, a) => total + a.minorUnits, 0);
  return { minorUnits, currency };
}

export const count = (value: number) => value.toLocaleString(LOCALE);

/** `Aug 2`, or `Aug 2, 2025` outside this year. */
export function shortDate(rfc3339: string | null, now = new Date()): string {
  if (!rfc3339) return '—';
  const date = new Date(rfc3339);
  const sameYear = date.getFullYear() === now.getFullYear();
  return date.toLocaleDateString(LOCALE, {
    month: 'short',
    day: 'numeric',
    ...(sameYear ? {} : { year: 'numeric' }),
  });
}

/** `Aug 1, 2026`. */
export function longDate(rfc3339: string | null): string {
  if (!rfc3339) return '—';
  return new Date(rfc3339).toLocaleDateString(LOCALE, {
    month: 'short',
    day: 'numeric',
    year: 'numeric',
  });
}

/** `2026-05` as `May 2026`. */
export function monthLabel(month: string): string {
  const [year, index] = month.split('-').map(Number);
  return new Date(Date.UTC(year, index - 1, 1)).toLocaleDateString(LOCALE, {
    month: 'short',
    year: 'numeric',
    timeZone: 'UTC',
  });
}

/** `2 min ago`. */
export function ago(rfc3339: string | null, now = Date.now()): string {
  if (!rfc3339) return 'never';
  const seconds = Math.round((new Date(rfc3339).getTime() - now) / 1000);
  const units: Array<[Intl.RelativeTimeFormatUnit, number]> = [
    ['day', 86_400],
    ['hour', 3_600],
    ['minute', 60],
  ];
  const format = new Intl.RelativeTimeFormat(LOCALE, { numeric: 'auto', style: 'short' });
  for (const [unit, size] of units) {
    if (Math.abs(seconds) >= size) return format.format(Math.round(seconds / size), unit);
  }
  return 'just now';
}

/** How often a newsletter arrives: `2×/day`, `daily`, `4×/wk`, `weekly`, `monthly`. */
export function frequency(perWeek: number | null, received: number): string {
  if (received < 2 || perWeek === null) return 'once';
  if (perWeek >= 10.5) return `${Math.round(perWeek / 7)}×/day`;
  if (perWeek >= 5.5) return 'daily';
  if (perWeek >= 1.5) return `${Math.round(perWeek)}×/wk`;
  if (perWeek >= 0.75) return 'weekly';
  const perMonth = (perWeek * 52) / 12;
  if (perMonth >= 1.5) return `${Math.round(perMonth)}×/mo`;
  if (perMonth >= 0.75) return 'monthly';
  return 'rarely';
}

/** A year's mail as a rate: `14/wk`, `3/mo`, `5/yr`. */
export function rate(perYear: number): string {
  if (perYear >= 52) return `${Math.round(perYear / 52)}/wk`;
  if (perYear >= 12) return `${Math.round(perYear / 12)}/mo`;
  return `${perYear}/yr`;
}

export function cadenceLabel(cadence: Cadence | null): string {
  return cadence ?? '—';
}

/** Two letters for an avatar: `NYTimes` → `NY`, `The Dispatch` → `TD`. */
export function initials(name: string): string {
  const words = name
    .replace(/[^\p{L}\p{N}\s]/gu, ' ')
    .split(/\s+/)
    .filter(Boolean);
  if (words.length >= 2) return (words[0][0] + words[1][0]).toUpperCase();
  return (words[0] ?? '?').slice(0, 2).toUpperCase();
}

export function bytes(value: number | null): string {
  if (value === null) return '';
  const units = ['B', 'KB', 'MB', 'GB', 'TB'];
  let size = value;
  let unit = 0;
  while (size >= 1024 && unit < units.length - 1) {
    size /= 1024;
    unit++;
  }
  return `${size.toFixed(unit === 0 || size >= 10 ? 0 : 1)} ${units[unit]}`;
}
