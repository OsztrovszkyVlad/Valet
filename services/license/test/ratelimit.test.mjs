import { test } from 'node:test';
import assert from 'node:assert/strict';
import http from 'node:http';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { generateKeyPair } from '../lib/signing.mjs';
import { createJsonFileStore } from '../lib/store.mjs';
import { createHandler } from '../lib/handler.mjs';
import { createRateLimiter } from '../lib/ratelimit.mjs';

const key = generateKeyPair();

test('token bucket: N requests pass, then retryAfter, refill over time', () => {
  let t = 0;
  const rl = createRateLimiter({ capacity: 3, perMinute: 3, nowMs: () => t });
  for (let i = 0; i < 3; i++) assert.deepEqual(rl.take('a'), { ok: true });
  assert.deepEqual(rl.take('a'), { ok: false, retryAfter: 20 });
  t += 19_000;
  assert.equal(rl.take('a').ok, false);
  t += 1_000;
  assert.equal(rl.take('a').ok, true);
  assert.equal(rl.take('a').ok, false);
});

test('token bucket: clients are limited separately', () => {
  const rl = createRateLimiter({ capacity: 2, perMinute: 2, nowMs: () => 0 });
  assert.equal(rl.take('a').ok, true);
  assert.equal(rl.take('a').ok, true);
  assert.equal(rl.take('a').ok, false);
  assert.equal(rl.take('b').ok, true);
  assert.equal(rl.take('b').ok, true);
  assert.equal(rl.take('b').ok, false);
});

test('token bucket: refilled buckets are pruned and the map stays bounded', () => {
  let t = 0;
  const rl = createRateLimiter({ capacity: 2, perMinute: 2, maxEntries: 5, nowMs: () => t });
  for (let i = 0; i < 4; i++) rl.take(`c${i}`);
  assert.equal(rl.size(), 4);
  t += 61_000; // every bucket refilled, and the sweep interval has passed
  rl.take('fresh');
  assert.equal(rl.size(), 1, 'full buckets removed');
  for (let i = 0; i < 100; i++) {
    rl.take(`burst${i}`);
    assert.ok(rl.size() <= 5, `size ${rl.size()} after ${i}`);
  }
  // A client that keeps a partly used bucket still gets limited while it stays in the map.
  const rl2 = createRateLimiter({ capacity: 1, perMinute: 1, maxEntries: 1000, nowMs: () => t });
  rl2.take('x');
  assert.equal(rl2.take('x').ok, false);
});

async function serve(options) {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'valet-license-rl-'));
  const server = http.createServer(
    createHandler({ store: createJsonFileStore(path.join(dir, 's.json')), key, log: () => {}, ...options }),
  );
  await new Promise((resolve) => server.listen(0, '127.0.0.1', resolve));
  const base = `http://127.0.0.1:${server.address().port}`;
  const call = async (route, headers = {}) => {
    const res = await fetch(`${base}${route}`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json', ...headers },
      body: JSON.stringify({ license_id: '6f1c2d3e-4b5a-4c6d-9e8f-0a1b2c3d4e5f', machine: '0'.repeat(32), app_version: '1' }),
    });
    await res.text();
    return res;
  };
  return { base, call, close: () => new Promise((resolve) => server.close(resolve)) };
}

test('activation routes answer 429 with Retry-After after N requests per client', async () => {
  const s = await serve({ rateLimiter: createRateLimiter({ capacity: 3, perMinute: 3 }) });
  try {
    for (const route of ['/v1/activate', '/v1/validate', '/v1/deactivate']) {
      assert.equal((await s.call(route)).status, 404, route); // unknown license, but not limited
    }
    const r = await s.call('/v1/activate');
    assert.equal(r.status, 429);
    assert.equal(r.headers.get('retry-after'), '20');
    assert.deepEqual(await (await fetch(`${s.base}/v1/validate`, { method: 'POST', body: '{}' })).json(), { error: 'rate_limited' });
    const health = await fetch(`${s.base}/healthz`);
    assert.equal(health.status, 200, '/healthz is not limited');
  } finally {
    await s.close();
  }
});

test('Fly-Client-IP is used only when trusted', async () => {
  const trusted = await serve({ rateLimiter: createRateLimiter({ capacity: 1, perMinute: 1 }), trustFly: true });
  try {
    assert.equal((await trusted.call('/v1/activate', { 'Fly-Client-IP': '203.0.113.1' })).status, 404);
    assert.equal((await trusted.call('/v1/activate', { 'Fly-Client-IP': '203.0.113.1' })).status, 429);
    assert.equal((await trusted.call('/v1/activate', { 'Fly-Client-IP': '203.0.113.2' })).status, 404, 'separate per IP');
  } finally {
    await trusted.close();
  }
  const untrusted = await serve({ rateLimiter: createRateLimiter({ capacity: 1, perMinute: 1 }) });
  try {
    assert.equal((await untrusted.call('/v1/activate', { 'Fly-Client-IP': '203.0.113.1' })).status, 404);
    assert.equal((await untrusted.call('/v1/activate', { 'Fly-Client-IP': '203.0.113.2' })).status, 429, 'header ignored: same socket address');
  } finally {
    await untrusted.close();
  }
});

test('webhooks are not rate limited', async () => {
  const s = await serve({
    rateLimiter: createRateLimiter({ capacity: 1, perMinute: 1 }),
    webhook: { merchant: 'stripe', secrets: ['test-secret'], mail: { apiKey: 'test-resend-key', from: 'Valet <a@valetapp.ro>' }, fetch: async () => { throw new Error('no fetch expected'); } },
  });
  try {
    for (let i = 0; i < 5; i++) {
      const res = await fetch(`${s.base}/v1/webhook/stripe`, { method: 'POST', body: '{}' });
      assert.equal(res.status, 400, 'bad signature, not 429');
    }
  } finally {
    await s.close();
  }
});

test('/healthz answers 200 ok to GET and HEAD, 405 otherwise', async () => {
  const s = await serve({});
  try {
    const get = await fetch(`${s.base}/healthz`);
    assert.equal(get.status, 200);
    assert.equal(await get.text(), 'ok');
    assert.equal(get.headers.get('cache-control'), 'no-store');
    const head = await fetch(`${s.base}/healthz`, { method: 'HEAD' });
    assert.equal(head.status, 200);
    const post = await fetch(`${s.base}/healthz`, { method: 'POST', body: 'x' });
    assert.equal(post.status, 405);
  } finally {
    await s.close();
  }
});
