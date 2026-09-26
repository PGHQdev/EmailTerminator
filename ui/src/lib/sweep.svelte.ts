import { goto } from '$app/navigation';
import { Channel } from '@tauri-apps/api/core';
import { commands, type Event, type Item, type ItemResult, type Target } from '$lib/bindings';

export type RowState = 'queued' | 'running' | 'done';

/** An S11 row: the reviewed item and where the sweep has got to with it. */
export interface Row extends Item {
  state: RowState;
  /** The user confirmed this critical item in S10. */
  confirmed: boolean;
  result: ItemResult | null;
}

let rows = $state<Row[]>([]);
let running = $state(false);
let stopping = $state(false);
let problem = $state<string | null>(null);
/** S07's rows after a sweep that ran without S11. */
let result = $state<Row[] | null>(null);
/** The critical item S10 is asking about. */
let asking = $state<{
  name: string;
  action: 'unsubscribe' | 'cancel';
  answer: (yes: boolean) => void;
} | null>(null);
/** Bumped after every sweep, so lists reload what changed. */
let version = $state(0);

/** S10: resolves true when the user typed the phrase and confirmed. */
function confirm(name: string, action: 'unsubscribe' | 'cancel' = 'unsubscribe'): Promise<boolean> {
  return new Promise((resolve) => {
    asking = {
      name,
      action,
      answer: (yes) => {
        asking = null;
        resolve(yes);
      },
    };
  });
}

/** Every included critical row needs its own yes before it runs. */
async function confirmCritical() {
  for (const row of rows) {
    if (row.included && row.critical && !row.confirmed) {
      row.confirmed = await confirm(row.name);
      row.included = row.confirmed;
    }
  }
}

async function execute() {
  const pending = rows.flatMap((row) => (row.included && row.state === 'queued' ? [row] : []));
  if (pending.length === 0) return;
  const events = new Channel<Event>();
  events.onmessage = (event) => {
    const row = pending[event.index];
    if (event.kind === 'started') {
      row.state = 'running';
    } else {
      row.state = 'done';
      row.result = event.result;
    }
  };
  problem = null;
  running = true;
  const run = await commands.runSweep(
    pending.map((row) => ({ target: row.target, criticalConfirmed: row.confirmed })),
    events,
  );
  running = false;
  stopping = false;
  version += 1;
  if (run.status === 'error') problem = run.error;
}

/**
 * Starts a sweep over `targets`: S11 first when the settings ask for it,
 * otherwise straight to the run and S07's result.
 */
async function begin(targets: Target[]) {
  if (running || targets.length === 0) return;
  problem = null;
  const [settings, review] = await Promise.all([
    commands.sweepSettings(),
    commands.sweepReview(targets),
  ]);
  if (review.status === 'error') {
    problem = review.error;
    return;
  }
  rows = review.data.map((item) => ({ ...item, state: 'queued', confirmed: false, result: null }));
  if (settings.confirmAlways || rows.length >= settings.confirmFrom) {
    await goto('/sweep');
    return;
  }
  // Without S11 nothing is excluded quietly: each critical item asks.
  for (const row of rows) if (row.critical) row.included = true;
  await confirmCritical();
  if (!rows.some((row) => row.included)) return;
  await execute();
  result = rows.filter((row) => row.included);
}

export const sweep = {
  get rows() {
    return rows;
  },
  get running() {
    return running;
  },
  get stopping() {
    return stopping;
  },
  get problem() {
    return problem;
  },
  get result() {
    return result;
  },
  get asking() {
    return asking;
  },
  get version() {
    return version;
  },
  begin,
  /** S10 for one action outside a sweep, such as opening a playbook (S08). */
  confirm,
  /** S11's checkbox. Including a critical row asks S10 first. */
  async toggle(row: Row) {
    if (running || row.state !== 'queued') return;
    if (row.included) {
      row.included = false;
      row.confirmed = false;
    } else if (row.critical) {
      row.confirmed = await confirm(row.name);
      row.included = row.confirmed;
    } else {
      row.included = true;
    }
  },
  /** S11's run button; after a stop it runs what is still queued. */
  async run() {
    if (running) return;
    await confirmCritical();
    await execute();
  },
  stop() {
    stopping = true;
    commands.stopSweep();
  },
  closeResult() {
    result = null;
    problem = null;
  },
};
