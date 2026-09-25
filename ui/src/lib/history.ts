import type { Amount, MonthSpend, PriceMove } from '$lib/bindings';

const MONTH = /^(\d{4})-(\d{2})/;

function index(month: string): number {
  const [, y, m] = MONTH.exec(month) ?? ['', '1970', '01'];
  return Number(y) * 12 + Number(m) - 1;
}

/**
 * S06's spend history as a stepped line in a 640 × 120 box: each month with a
 * charge sets the level, which holds until the next one, up to this month.
 * Levels are the distinct prices in order, for the labels under the chart.
 */
export function stepLine(spend: MonthSpend[], currency: string, now = new Date()) {
  const points = spend.filter((s) => s.amount.currency === currency);
  if (points.length === 0) return null;
  const first = index(points[0].month);
  const last = Math.max(now.getUTCFullYear() * 12 + now.getUTCMonth(), index(points.at(-1)!.month));
  const span = Math.max(1, last - first);
  const top = Math.max(...points.map((p) => p.amount.minorUnits), 1);
  const x = (month: string) => ((index(month) - first) / span) * 640;
  const y = (minor: number) => 112 - (minor / top) * 90;

  let line = '';
  const levels: Amount[] = [];
  points.forEach((p, i) => {
    const px = x(p.month).toFixed(1);
    const py = y(p.amount.minorUnits).toFixed(1);
    line += i === 0 ? `M${px} ${py}` : ` L${px} ${y(points[i - 1].amount.minorUnits).toFixed(1)} L${px} ${py}`;
    if (levels.at(-1)?.minorUnits !== p.amount.minorUnits) levels.push(p.amount);
  });
  line += ` L640 ${y(points.at(-1)!.amount.minorUnits).toFixed(1)}`;
  return { line, area: `${line} L640 120 L0 120 Z`, levels: levels.slice(-6), from: points[0].month };
}

/** The price rise over the last twelve months, in percent, or null. */
export function priceRise(changes: PriceMove[], now: number): number | null {
  const recent = changes.filter((c) => now - Date.parse(c.at) <= 365 * 86_400_000);
  if (recent.length === 0) return null;
  const from = recent[0].from.minorUnits;
  const to = recent.at(-1)!.to.minorUnits;
  if (from <= 0 || to <= from || recent[0].from.currency !== recent.at(-1)!.to.currency) return null;
  return Math.round(((to - from) / from) * 100);
}
