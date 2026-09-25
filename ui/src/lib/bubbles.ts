/** S03's circles: one per service, sized by what it costs, as the mockup does. */

export interface Placed<T> {
  item: T;
  /** Centre and diameter, in the field's units. */
  x: number;
  y: number;
  d: number;
}

export const FIELD = { width: 620, height: 640 };
const LARGEST = 195;
const SMALLEST = 76;
const GAP = 10;

/**
 * Places the circles largest first: each takes the free spot nearest the
 * centre, searched ring by ring outward. Deterministic, so a list lays out the
 * same way every time. A circle that finds no room is left out.
 */
export function pack<T>(items: T[], weight: (item: T) => number): Placed<T>[] {
  const max = Math.max(...items.map(weight), 1);
  const sized = items
    .map((item) => ({ item, d: Math.max(SMALLEST, (LARGEST * Math.max(weight(item), 0)) / max) }))
    .sort((a, b) => b.d - a.d);

  const centre = { x: FIELD.width / 2, y: FIELD.height / 2 };
  const reach = Math.hypot(centre.x, centre.y);
  const placed: Placed<T>[] = [];
  for (const { item, d } of sized) {
    const r = d / 2;
    const spot = (): { x: number; y: number } | null => {
      for (let distance = 0; distance <= reach; distance += 4) {
        const steps = Math.max(1, Math.ceil((2 * Math.PI * distance) / 4));
        for (let i = 0; i < steps; i++) {
          const angle = (i / steps) * 2 * Math.PI;
          const x = centre.x + Math.cos(angle) * distance;
          const y = centre.y + Math.sin(angle) * distance;
          const inside =
            x - r >= 0 && y - r >= 0 && x + r <= FIELD.width && y + r <= FIELD.height;
          if (inside && placed.every((p) => Math.hypot(p.x - x, p.y - y) >= (p.d + d) / 2 + GAP)) {
            return { x, y };
          }
        }
      }
      return null;
    };
    const found = spot();
    if (found) placed.push({ item, d, ...found });
  }
  return placed;
}
