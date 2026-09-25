export type ThemeSetting = 'light' | 'dark' | 'system';

/** `system` follows the OS preference (PLAN.md 2.4). */
export function resolveTheme(setting: ThemeSetting, prefersDark: boolean): 'light' | 'dark' {
  if (setting === 'system') return prefersDark ? 'dark' : 'light';
  return setting;
}
