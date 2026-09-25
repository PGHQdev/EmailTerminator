<script lang="ts">
  import { goto } from '$app/navigation';
  import type { Amount } from '$lib/bindings';
  import { FIELD, pack } from '$lib/bubbles';
  import { money, sum } from '$lib/format';

  interface Tile {
    id: number;
    name: string;
    monthly: Amount | null;
    priceIncrease: boolean;
    isCritical: boolean;
  }

  // S03's field: the dearest services as circles, the rest in one.
  let { tiles, currency }: { tiles: Tile[]; currency: string | null } = $props();

  const SHOWN = 9;
  const TONES = [1, 2, 1, 3, 4, 2, 3, 4, 1];

  type Bubble = { tile: Tile | null; name: string; amount: string; tone: number; cost: number };

  let bubbles = $derived.by(() => {
    const cost = (t: Tile) => (t.monthly && t.monthly.currency === currency ? t.monthly.minorUnits : 0);
    const shown: Bubble[] = tiles.slice(0, SHOWN).map((tile, i) => ({
      tile,
      name: tile.name + (tile.isCritical ? ' ⚠' : ''),
      amount: tile.monthly ? money(tile.monthly) + (tile.priceIncrease ? ' ↑' : '') : 'irregular',
      tone: TONES[i % TONES.length],
      cost: cost(tile),
    }));
    const rest = tiles.slice(SHOWN);
    if (rest.length > 0) {
      const total = currency ? sum(rest.flatMap((t) => (t.monthly ? [t.monthly] : [])), currency) : null;
      shown.push({
        tile: null,
        name: `+${rest.length} more`,
        amount: total && total.minorUnits > 0 ? money(total) : '',
        tone: 4,
        cost: 0,
      });
    }
    return pack(shown, (b) => b.cost);
  });

  const pct = (value: number, of: number) => `${(value / of) * 100}%`;
</script>

<div class="field">
  {#each bubbles as { item, x, y, d } (item.tile?.id ?? "more")}
    <button
      type="button"
      class="bubble tone{item.tone}"
      style:left={pct(x - d / 2, FIELD.width)}
      style:top={pct(y - d / 2, FIELD.height)}
      style:width={pct(d, FIELD.width)}
      style:--name={Math.max(13, d * 0.155) / FIELD.width}
      style:--amount={Math.max(11, d * 0.095) / FIELD.width}
      onclick={() => (item.tile ? goto(`/service?id=${item.tile.id}`) : goto('/subscriptions'))}
    >
      <span class="name">{item.name}</span>
      <span class="amount">{item.amount}</span>
    </button>
  {/each}
</div>

<style>
  .field {
    position: relative;
    width: 100%;
    aspect-ratio: 620 / 640;
    container-type: inline-size;
  }

  .bubble {
    position: absolute;
    aspect-ratio: 1;
    border: none;
    border-radius: var(--pill);
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    padding: 0 0.5rem;
    text-align: center;
    cursor: pointer;
    box-shadow: var(--shadow-md);
    transition: transform 0.18s;
  }

  .bubble:hover {
    transform: scale(1.06);
  }

  .bubble:active {
    transform: scale(1.02);
  }

  .name {
    font-family: var(--font-heading);
    font-weight: var(--font-heading-weight);
    font-size: calc(var(--name) * 100cqw);
    line-height: 1.15;
  }

  .amount {
    font-family: var(--font-body);
    font-size: calc(var(--amount) * 100cqw);
    opacity: 0.82;
  }

  .tone1 {
    background: var(--b1);
    color: var(--b1f);
  }

  .tone2 {
    background: var(--b2);
    color: var(--b2f);
  }

  .tone3 {
    background: var(--b3);
    color: var(--b3f);
  }

  .tone4 {
    background: var(--b4);
    color: var(--b4f);
  }
</style>
