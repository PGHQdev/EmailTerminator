import { commands, type Appearance } from '$lib/bindings';

let current = $state<Appearance>('system');

/** S17's appearance setting; the root layout applies it to `<html>`. */
export const appearance = {
  get value(): Appearance {
    return current;
  },
  async load() {
    current = await commands.appearance();
  },
  async set(value: Appearance) {
    current = value;
    await commands.setAppearance(value);
  },
};
