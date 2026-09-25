import type { AppInfo } from '$lib/bindings';

export const ISSUES = 'https://github.com/PGHQdev/EmailTerminator/issues/new';

/**
 * S15's issue, exactly as it is posted: the user's words, the app version and
 * the OS. Nothing from the mailbox goes in.
 */
export function issue(description: string, info: AppInfo) {
  const text = description.trim();
  const firstLine = text.split('\n')[0].trim();
  const title = firstLine.length > 72 ? `${firstLine.slice(0, 71)}…` : firstLine || 'Issue report';
  const body = [
    '**Describe the bug**',
    text || '(no description)',
    '',
    '**Environment**',
    `- EmailTerminator ${info.version} (${info.arch})`,
    `- ${info.os}`,
  ].join('\n');
  const url = `${ISSUES}?${new URLSearchParams({ title, body })}`;
  return { title, body, url };
}
