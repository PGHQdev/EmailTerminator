// Enforces the design-system adherence rules on .svelte files (PLAN.md 2.4):
// no raw hex colour, no raw px value, no font outside the system. oxlint has
// no rule that reaches CSS, so this script does it.
const rules: Array<[RegExp, string]> = [
  [/#[0-9a-fA-F]{3,8}\b/, 'raw hex colour: use an alias from theme.css'],
  [/\b\d+(\.\d+)?px\b/, 'raw px value: use a token from tokens.css'],
  [/font-family\s*:(?!\s*var\()/, 'font outside the design system: use var(--font-heading) or var(--font-body)'],
];

export function violations(source: string): Array<{ line: number; message: string }> {
  const found: Array<{ line: number; message: string }> = [];
  source.split('\n').forEach((text, index) => {
    for (const [pattern, message] of rules) {
      if (pattern.test(text)) found.push({ line: index + 1, message });
    }
  });
  return found;
}

if (import.meta.main) {
  const { Glob } = await import('bun');
  let failures = 0;
  for await (const path of new Glob('src/**/*.svelte').scan('.')) {
    for (const { line, message } of violations(await Bun.file(path).text())) {
      console.error(`${path}:${line}: ${message}`);
      failures++;
    }
  }
  if (failures > 0) process.exit(1);
}
