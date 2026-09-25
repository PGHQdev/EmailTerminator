// Development only: answers the Tauri commands with the mockups' figures, so
// `bun run dev` shows every screen in a plain browser for design review.
// `+layout.ts` loads this file only in dev and only outside Tauri, so a build
// never contains it. Add `?mock=empty` or `?mock=clean` for the S16 empty
// states, `?mock=locked` for the lost key, and `?theme=dark` for dark.
// `?demo=review`, `running`, `result` or `critical` opens S11, S11 mid-run,
// S07 or S10 with a sample sweep; `?demo=evidence` opens an evidence link.

import { mockIPC } from '@tauri-apps/api/mocks';
import type { Channel } from '@tauri-apps/api/core';
import type {
  Amount,
  Dashboard,
  Entry,
  Event,
  Item,
  Newsletter,
  RunItem,
  ServiceDetail,
  Subscription,
  Sweep,
  Target,
} from '$lib/bindings';

const params = new URLSearchParams(location.search);
for (const key of ['mock', 'theme', 'demo']) {
  const value = params.get(key);
  if (value) sessionStorage.setItem(key, value);
}
const mode = sessionStorage.getItem('mock') ?? 'full';
let theme = sessionStorage.getItem('theme') ?? 'system';
const demo = params.get('demo');
let sweepSettings: Sweep = {
  confirmAlways: demo !== 'result' && demo !== 'critical',
  confirmFrom: 10,
  excludeCritical: true,
};

const usd = (dollars: number): Amount => ({ minorUnits: Math.round(dollars * 100), currency: 'USD' });

const subscriptions: Subscription[] = [
  ['Netflix', 22.99, 'Aug 2', 96],
  ['Adobe Creative Cloud', 19.99, 'Jul 28', 124],
  ['Hulu', 17.99, 'Aug 5', 212, { price: true }],
  ['NYTimes', 17.0, 'Aug 1', 390, { price: true }],
  ['Audible', 14.95, 'Jul 22', 48],
  ['Spotify', 11.99, 'Aug 9', 36],
  ['Dropbox', 11.99, 'Aug 11', 22, { critical: true }],
  ['iCloud+', 9.99, 'Aug 3', 18, { critical: true }],
  ['Scribd', 9.99, 'Jun 30', 77, { status: 'canceling' }],
  ['Patreon', 8.0, 'Aug 1', 64],
  ['Duolingo', 6.99, 'Jul 14', 150, { price: true }],
  ['Notion', 4.0, 'Aug 8', 12],
  ['1Password', 2.99, 'Jul 19', 6, { critical: true, annual: true }],
  ['Medium', 3.13, 'Mar 2', 40, { annual: true }],
].map(([name, cost, last, volume, o = {}], i) => {
  const flags = o as { price?: boolean; critical?: boolean; status?: string; annual?: boolean };
  return {
    id: i + 1,
    name: name as string,
    monthly: usd(cost as number),
    cadence: flags.annual ? 'annual' : 'monthly',
    lastChargeAt: new Date(`${last} 2026 12:00 UTC`).toISOString(),
    emailsPerYear: volume as number,
    priceIncrease: !!flags.price,
    isCritical: !!flags.critical,
    status: (flags.status ?? 'active') as Subscription['status'],
  };
});

const newsletters: Newsletter[] = [
  ['The Dispatch', 'newsletter@thedispatch.com', 14, 728, true],
  ['LinkedIn', 'notifications@linkedin.com', 11, 572, true],
  ['Groupon', 'deals@groupon.com', 9, 468, true],
  ['Medium Daily Digest', 'digest@medium.com', 7, 364, true],
  ['REI Co-op', 'gearmail@rei.com', 4, 208, true],
  ['Bandcamp Weekly', 'noreply@bandcamp.com', 1, 52, false],
  ['Kickstarter', 'hello@kickstarter.com', 0.94, 49, true],
  ['Stratechery', 'email@stratechery.com', 3, 156, false],
  ['Morning Brew', 'crew@morningbrew.com', 5, 260, true],
].map(([name, address, perWeek, received, oneClick], i) => ({
  id: 100 + i,
  name: name as string,
  address: address as string,
  received: received as number,
  lastYear: received as number,
  serviceId: name === 'Medium Daily Digest' ? 14 : null,
  perWeek: perWeek as number,
  oneClick: oneClick as boolean,
  unsubscribedAt: null,
}));

const spend = (list: Subscription[]) =>
  usd(list.filter((s) => s.status !== 'canceled').reduce((t, s) => t + s.monthly!.minorUnits, 0) / 100);

function dashboard(): Dashboard {
  const empty = mode === 'empty';
  const clean = mode === 'clean';
  const subs = empty || clean ? [] : subscriptions;
  return {
    sources: empty ? 0 : 2,
    lastSyncAt: empty ? null : new Date(Date.now() - 2 * 60_000).toISOString(),
    scanned: empty ? 0 : 48_392,
    monthlySpend: subs.length ? [spend(subs), { minorUnits: 999, currency: 'EUR' }] : [],
    subscriptions: subs.length,
    newsletters: empty ? 0 : 63,
    emailsPerYear: empty ? 0 : 6840,
    services: subs.map((s) => ({
      id: s.id,
      name: s.name,
      monthly: s.monthly,
      priceIncrease: s.priceIncrease,
      isCritical: s.isCritical,
    })),
    loudest: empty
      ? []
      : newsletters.slice(0, 4).map((n) => ({ senderId: n.id, name: n.name, lastYear: n.lastYear })),
    cancelAll: subs.length ? [spend(subs.filter((s) => !s.isCritical))] : [],
    unsubscribeAll: empty ? 0 : 6840,
  };
}

function detail(id: number): ServiceDetail | null {
  const s = subscriptions.find((x) => x.id === id);
  if (!s) return null;
  const levels = [8.0, 9.99, 13.0, 17.0];
  const months: string[] = [];
  for (let y = 2021; y <= 2026; y++) {
    for (let m = 1; m <= 12; m++) {
      if ((y === 2021 && m < 3) || (y === 2026 && m > 8)) continue;
      months.push(`${y}-${String(m).padStart(2, '0')}`);
    }
  }
  const level = (month: string) =>
    month < '2022-08' ? levels[0] : month < '2024-01' ? levels[1] : month < '2026-05' ? levels[2] : levels[3];
  const bars = [34, 48, 28, 55, 62, 40, 71, 58, 66, 45, 78, 82];
  return {
    id: s.id,
    name: s.name,
    status: s.status,
    isCritical: s.isCritical,
    priceIncrease: s.priceIncrease,
    cadence: s.cadence,
    monthly: s.monthly,
    senders: [`billing@${s.name.toLowerCase().replace(/[^a-z0-9]/g, '')}.com`],
    firstSeen: '2021-03-01T09:00:00Z',
    lastCharge: { at: s.lastChargeAt!, amount: s.monthly! },
    spend: months.map((month) => ({ month, amount: usd(level(month)) })),
    volume: bars.map((messages, i) => ({
      month: `${i < 4 ? 2025 : 2026}-${String(((i + 8) % 12) + 1).padStart(2, '0')}`,
      messages,
    })),
    emailsPerYear: 390,
    receiptsPerYear: 12,
    priceChanges: [
      { at: '2022-08-01T09:00:00Z', from: usd(8), to: usd(9.99) },
      { at: '2024-01-01T09:00:00Z', from: usd(9.99), to: usd(13) },
      { at: '2026-05-01T09:00:00Z', from: usd(13), to: usd(17) },
    ],
    receipts: ['2026-08-01', '2026-07-01', '2026-06-01', '2026-05-01'].map((day, i) => ({
      messageId: i + 1,
      date: `${day}T09:00:00Z`,
      subject: `Your receipt from ${s.name}`,
      kind: 'charge',
      amount: s.monthly,
    })),
  };
}

function review(targets: Target[]): Item[] {
  return targets.flatMap((target): Item[] => {
    if (target.kind === 'sender') {
      const n = newsletters.find((x) => x.id === target.id);
      if (!n) return [];
      const route = n.oneClick ? 'oneClick' : 'page';
      return [{ target, name: n.name, emailsPerYear: n.lastYear, critical: false, route, senders: 1, included: true }];
    }
    const sub = subscriptions.find((x) => x.id === target.id);
    if (!sub) return [];
    const critical = sub.isCritical;
    return [{
      target,
      name: sub.name,
      emailsPerYear: Math.round(sub.emailsPerYear * 0.6),
      critical,
      route: 'oneClick',
      senders: 1,
      included: !(critical && sweepSettings.excludeCritical),
    }];
  });
}

let stopped = false;

/** Emits a started and a finished event per item, a beat apart. */
function run(items: RunItem[], events: Channel<Event>): Promise<number> {
  stopped = false;
  return new Promise((resolve) => {
    let index = 0;
    const next = () => {
      if (stopped || index >= items.length) return resolve(index);
      const i = index;
      const item = review([items[i].target])[0];
      events.onmessage({ kind: 'started', index: i });
      setTimeout(() => {
        const manual = item?.route === 'page';
        const failed = item?.name === 'Bandcamp Weekly';
        events.onmessage({
          kind: 'finished',
          index: i,
          result: {
            outcome: failed ? 'failed' : manual ? 'needsYou' : 'succeeded',
            detail: failed ? 'the sender answered 410' : manual ? 'no one-click unsubscribe' : 'the sender answered 200',
            actionIds: [1],
            finishAt: manual ? 'https://example.com/unsubscribe' : null,
          },
        });
        index += 1;
        next();
      }, 450);
    };
    next();
  });
}

const activity: Entry[] = [
  ['unsubscribe', 'The Dispatch', 0, 'succeeded', 'the sender answered 200', true],
  ['unsubscribe', 'LinkedIn', 0, 'succeeded', 'the sender answered 202', true],
  ['unsubscribe', 'Groupon', 0, 'succeeded', 'the sender answered 200', true],
  ['unsubscribe', 'Stratechery', 1, 'needsYou', 'no one-click unsubscribe: finish on the sender\'s page', false],
  ['unsubscribe', 'Bandcamp Weekly', 1, 'failed', 'no unsubscribe header in the mail', false],
  ['sync', 'me@example.com', 1, 'succeeded', '312 new messages', false],
  ['sync', 'me@example.org', 6, 'succeeded', '1,204 new messages', false],
].map(([kind, target, daysAgo, outcome, detail, post], i) => {
  const at = new Date(Date.now() - (daysAgo as number) * 86_400_000 - i * 60_000).toISOString();
  const name = target as string;
  return {
    id: i + 1,
    kind: kind as Entry['kind'],
    target: name,
    at,
    outcome: outcome as Entry['outcome'],
    detail: detail as string,
    request: post ? `POST https://${name.toLowerCase().replace(/\W/g, '')}.com/u/8a1f2c` : null,
    evidence:
      kind === 'sync'
        ? null
        : {
            messageId: i === 4 ? null : i + 1,
            subject: `${name}: this week's issue`,
            from: `${name} <news@${name.toLowerCase().replace(/\W/g, '')}.com>`,
            date: at,
          },
  };
});

if (demo === 'evidence') {
  setTimeout(async () => (await import('$lib/evidence.svelte')).evidence.open(1), 300);
} else if (demo) {
  // After the app has mounted: start the sample sweep the way a page would.
  setTimeout(async () => {
    const { sweep } = await import('$lib/sweep.svelte');
    const sample: Target[] =
      demo === 'critical'
        ? [{ kind: 'service', id: 8 }]
        : demo === 'result'
          ? [100, 101, 105].map((id) => ({ kind: 'sender', id }) as const)
          : [
              ...[100, 101, 102, 103, 104, 105, 108].map((id) => ({ kind: 'sender', id }) as const),
              ...[3, 5, 7, 8].map((id) => ({ kind: 'service', id }) as const),
            ];
    await sweep.begin(sample);
    if (demo === 'running') void sweep.run();
  }, 300);
}

mockIPC((cmd, args) => {
  const ok = <T>(data: T) => data;
  const a = (args ?? {}) as Record<string, unknown>;
  switch (cmd) {
    case 'store_status':
      return mode === 'locked'
        ? { state: 'locked' }
        : { state: 'open', keyBackend: 'OS keychain', cipherVersion: '4.6.1' };
    case 'list_sources':
      return mode === 'empty'
        ? []
        : [
            { id: 1, kind: 'imap', label: 'Gmail · me@example.com', lastSyncAt: null, messageCount: 41_210 },
            { id: 2, kind: 'imap', label: 'Fastmail · me@example.org', lastSyncAt: null, messageCount: 7_182 },
          ];
    case 'dashboard':
      return ok(dashboard());
    case 'subscriptions':
      return mode === 'full' ? subscriptions : [];
    case 'newsletters':
      return mode === 'empty' ? [] : newsletters;
    case 'service_detail':
      return detail(Number(a.id));
    case 'appearance':
      return theme;
    case 'set_appearance':
      theme = String(a.appearance);
      sessionStorage.setItem('theme', theme);
      return null;
    case 'data_location':
      return {
        path: '~/Library/Application Support/EmailTerminator',
        bytes: 3.1 * 1024 ** 3,
        keyPlace: 'keychain',
      };
    case 'app_info':
      return { version: '0.0.0', os: 'macOS 15.5', arch: 'aarch64' };
    case 'move_data_location':
      return { kind: 'cancelled' };
    case 'erase_local_data':
      throw 'Erasing is off in the design preview.';
    case 'imap_presets':
      return [];
    case 'sweep_settings':
      return sweepSettings;
    case 'set_sweep_settings':
      sweepSettings = a.sweep as Sweep;
      return null;
    case 'sweep_review':
      return review(a.targets as Target[]);
    case 'run_sweep':
      return run(a.items as RunItem[], a.events as Channel<Event>);
    case 'stop_sweep':
      stopped = true;
      return null;
    case 'activity':
      return mode === 'empty' ? [] : activity;
    case 'evidence_original':
      return Number(a.actionId) === 5
        ? { kind: 'gone' }
        : {
            kind: 'found',
            preview: {
              headers: [
                { name: 'From', value: 'The Dispatch <newsletter@thedispatch.com>' },
                { name: 'Date', value: 'Sat, 26 Sep 2026 06:30:00 +0000' },
                { name: 'Subject', value: 'The Dispatch: this week' },
                { name: 'List-Unsubscribe', value: '<https://thedispatch.com/u/8a1f2c>' },
                { name: 'List-Unsubscribe-Post', value: 'List-Unsubscribe=One-Click' },
              ],
              text: 'Good morning.\n\nThis week: three stories worth your time.',
            },
          };
    default:
      console.info('[mock] unhandled command', cmd, args);
      return null;
  }
});
