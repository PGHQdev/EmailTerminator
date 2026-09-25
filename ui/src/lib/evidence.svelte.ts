import { commands, type Entry } from '$lib/bindings';

let entry = $state<Entry | null>(null);
let problem = $state<string | null>(null);

/** The activity row an evidence link opened; the dialog shows it. */
export const evidence = {
  get entry() {
    return entry;
  },
  get problem() {
    return problem;
  },
  async open(actionId: number) {
    problem = null;
    const log = await commands.activity();
    if (log.status === 'error') {
      problem = log.error;
      return;
    }
    entry = log.data.find((e) => e.id === actionId) ?? null;
  },
  show(row: Entry) {
    problem = null;
    entry = row;
  },
  close() {
    entry = null;
  },
};
