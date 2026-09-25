import { describe, expect, it } from 'vitest';
import app from './index';

describe('worker', () => {
  it('answers the health check', async () => {
    const res = await app.request('/health');
    expect(res.status).toBe(200);
    expect(await res.text()).toBe('ok');
  });

  it('has no other route before M8', async () => {
    expect((await app.request('/activate', { method: 'POST' })).status).toBe(404);
  });
});
