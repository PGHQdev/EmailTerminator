import { describe, expect, it } from 'vitest';
import { issue } from './issue';

const info = { version: '0.1.0', os: 'macOS 15.5', arch: 'aarch64' };

describe('issue', () => {
  it('takes the title from the first line and lists the environment', () => {
    const { title, body, url } = issue('Scan stops at 80%\nIt happened twice.', info);
    expect(title).toBe('Scan stops at 80%');
    expect(body).toContain('- EmailTerminator 0.1.0 (aarch64)\n- macOS 15.5');
    expect(new URL(url).searchParams.get('body')).toBe(body);
  });

  it('shortens a long first line and names an empty report', () => {
    expect(issue('x'.repeat(100), info).title).toHaveLength(72);
    expect(issue('   ', info).title).toBe('Issue report');
  });
});
