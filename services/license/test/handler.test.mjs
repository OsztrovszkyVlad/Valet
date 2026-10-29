import { describe, test, before, after, beforeEach } from 'node:test';
import assert from 'node:assert/strict';
import http from 'node:http';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { generateKeyPair, publicKeyLine } from '../lib/signing.mjs';
import { issueLicense, openEnvelope } from '../lib/license.mjs';
import { createJsonFileStore, registerLicense } from '../lib/store.mjs';
import { createHandler } from '../lib/handler.mjs';

const key = generateKeyPair();
const pub = publicKeyLine(key);
const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'valet-license-handler-'));
const store = createJsonFileStore(path.join(dir, 'store.json'));
let clock = 1_800_000_000;
const logs = [];
let server;
let base;

const machine = (n) => n.toString(16).padStart(32, '0');

describe('activation API', () => {
  before(async () => {
    server = http.createServer(createHandler({ store, key, now: () => clock, log: (e) => logs.push(e) }));
    await new Promise((resolve) => server.listen(0, '127.0.0.1', resolve));
    base = `http://127.0.0.1:${server.address().port}`;
  });

  after(async () => {
    await new Promise((resolve) => server.close(resolve));
  });

  beforeEach(() => {
    clock = 1_800_000_000;
  });

  async function newLicense(options = {}) {
    const { payload } = issueLicense({ email: 'buyer@example.com', ...options }, key);
    await registerLicense(store, payload);
    return payload.license_id;
  }

  async function post(route, body, headers = {}) {
    const res = await fetch(`${base}${route}`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json', ...headers },
      body: typeof body === 'string' ? body : JSON.stringify(body),
    });
    const text = await res.text();
    return { status: res.status, body: text ? JSON.parse(text) : undefined, headers: res.headers };
  }

  const activate = (id, m, v = '0.2.0') => post('/v1/activate', { license_id: id, machine: m, app_version: v });
  const validate = (id, m, v = '0.2.0') => post('/v1/validate', { license_id: id, machine: m, app_version: v });
  const deactivate = (id, m) => post('/v1/deactivate', { license_id: id, machine: m });

  test('activate returns a signed activation token', async () => {
    const id = await newLicense();
    const r = await activate(id, machine(1));
    assert.equal(r.status, 200);
    assert.equal(r.headers.get('cache-control'), 'no-store');
    assert.deepEqual(Object.keys(r.body), ['token']);
    assert.equal(typeof r.body.token.payload, 'string');
    const t = openEnvelope(r.body.token, pub);
    assert.deepEqual(t, { v: 1, kind: 'activation', license_id: id, machine: machine(1), checked_at: clock });
  });

  test('3 machines activate, the 4th gets 409 too_many', async () => {
    const id = await newLicense();
    for (const n of [1, 2, 3]) assert.equal((await activate(id, machine(n))).status, 200);
    const r = await activate(id, machine(4));
    assert.equal(r.status, 409);
    assert.deepEqual(r.body, { error: 'too_many' });
  });

  test('re-activating the same machine returns a fresh token without using a slot', async () => {
    const id = await newLicense();
    for (const n of [1, 2, 3]) await activate(id, machine(n));
    clock += 100;
    const r = await activate(id, machine(2));
    assert.equal(r.status, 200);
    assert.equal(openEnvelope(r.body.token, pub).checked_at, clock);
    const rec = await store.get(id);
    assert.equal(Object.values(rec.machines).filter((m) => m.active).length, 3);
    assert.equal(rec.machines[machine(2)].first_seen, clock - 100);
    assert.equal(rec.machines[machine(2)].last_seen, clock);
  });

  test('deactivate frees the slot and keeps history', async () => {
    const id = await newLicense();
    for (const n of [1, 2, 3]) await activate(id, machine(n));
    clock += 50;
    const d = await deactivate(id, machine(1));
    assert.equal(d.status, 204);
    assert.equal(d.body, undefined);
    assert.equal((await deactivate(id, machine(1))).status, 204, 'deactivate is idempotent');
    const rec = await store.get(id);
    assert.equal(rec.machines[machine(1)].active, false);
    assert.equal(rec.machines[machine(1)].deactivated_at, clock);
    assert.equal(rec.machines[machine(1)].first_seen, clock - 50);
    assert.equal((await activate(id, machine(4))).status, 200);
    assert.equal((await activate(id, machine(1))).status, 409);
  });

  test('revoked license answers 410 on activate and validate', async () => {
    const id = await newLicense();
    await activate(id, machine(1));
    const rec = await store.get(id);
    rec.revoked = true;
    await store.put(id, rec);
    for (const call of [activate, validate]) {
      const r = await call(id, machine(1));
      assert.equal(r.status, 410);
      assert.deepEqual(r.body, { error: 'revoked' });
    }
    assert.equal((await deactivate(id, machine(1))).status, 204);
  });

  test('unknown license answers 404 unknown on all three calls', async () => {
    const id = '6f1c2d3e-4b5a-4c6d-9e8f-0a1b2c3d4e5f';
    for (const call of [activate, validate]) {
      const r = await call(id, machine(1));
      assert.equal(r.status, 404);
      assert.deepEqual(r.body, { error: 'unknown' });
    }
    assert.deepEqual((await deactivate(id, machine(1))).body, { error: 'unknown' });
  });

  test('validate refreshes checked_at and last_seen', async () => {
    const id = await newLicense();
    await activate(id, machine(1));
    clock += 31 * 86_400;
    const r = await validate(id, machine(1));
    assert.equal(r.status, 200);
    assert.equal(openEnvelope(r.body.token, pub).checked_at, clock);
    assert.equal((await store.get(id)).machines[machine(1)].last_seen, clock);
  });

  test('validate for a machine not in the active set re-activates it or answers 409', async () => {
    const id = await newLicense();
    const r = await validate(id, machine(9));
    assert.equal(r.status, 200);
    for (const n of [1, 2]) await activate(id, machine(n));
    const full = await validate(id, machine(5));
    assert.equal(full.status, 409);
    assert.deepEqual(full.body, { error: 'too_many' });
  });

  test('max_activations comes from the license record', async () => {
    const id = await newLicense({ maxActivations: 1 });
    assert.equal((await activate(id, machine(1))).status, 200);
    assert.equal((await activate(id, machine(2))).status, 409);
  });

  test('concurrent activations never exceed max_activations', async () => {
    const id = await newLicense();
    const results = await Promise.all([1, 2, 3, 4, 5, 6].map((n) => activate(id, machine(n))));
    assert.equal(results.filter((r) => r.status === 200).length, 3);
    assert.equal(results.filter((r) => r.status === 409).length, 3);
  });

  test('malformed inputs answer 400', async () => {
    const id = await newLicense();
    const good = { license_id: id, machine: machine(1), app_version: '0.2.0' };
    const bad = [
      '{not json',
      '[]',
      'null',
      '"str"',
      {},
      { ...good, license_id: 'abc' },
      { ...good, license_id: id.toUpperCase() },
      { ...good, license_id: '6f1c2d3e-4b5a-1c6d-9e8f-0a1b2c3d4e5f' }, // version 1 UUID
      { ...good, machine: machine(0xabcdef).toUpperCase() },
      { ...good, machine: 'abc' },
      { ...good, machine: `${machine(1)}0` },
      { ...good, machine: 123 },
      { ...good, app_version: '' },
      { ...good, app_version: 'x'.repeat(33) },
      { ...good, app_version: '1.0 beta' },
      { ...good, app_version: 2 },
      { license_id: id, machine: machine(1) },
      { ...good, email: 'a@b.c' },
    ];
    for (const body of bad) {
      const r = await post('/v1/activate', body);
      assert.equal(r.status, 400, `expected 400 for ${JSON.stringify(body)}`);
      assert.deepEqual(r.body, { error: 'bad_request' });
    }
    assert.equal((await post('/v1/deactivate', { license_id: id })).status, 400);
    assert.equal((await post('/v1/deactivate', { ...good, extra: 1 })).status, 400);
    assert.equal((await post('/v1/deactivate', good)).status, 204, 'app_version is optional on deactivate');
    const invalidUtf8 = await fetch(`${base}/v1/activate`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: Buffer.from([0x7b, 0xff, 0x7d]),
    });
    assert.equal(invalidUtf8.status, 400);
  });

  test('non-JSON content type answers 415', async () => {
    const id = await newLicense();
    const body = JSON.stringify({ license_id: id, machine: machine(1), app_version: '0.2.0' });
    assert.equal((await post('/v1/activate', body, { 'Content-Type': 'text/plain' })).status, 415);
    assert.equal(
      (await post('/v1/activate', body, { 'Content-Type': 'application/json; charset=utf-8' })).status,
      200,
    );
  });

  test('body over 4 KB answers 413', async () => {
    const id = await newLicense();
    const body = JSON.stringify({ license_id: id, machine: machine(1), app_version: '0.2.0', pad: 'x'.repeat(5000) });
    const r = await post('/v1/activate', body);
    assert.equal(r.status, 413);
    assert.deepEqual(r.body, { error: 'too_large' });
    // Chunked body without Content-Length.
    const chunked = await new Promise((resolve, reject) => {
      const req = http.request(`${base}/v1/activate`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json', 'Transfer-Encoding': 'chunked' },
      });
      req.on('response', (res) => {
        res.resume();
        resolve(res.statusCode);
      });
      req.on('error', reject);
      for (let i = 0; i < 5; i++) req.write('x'.repeat(1000));
      req.end();
    });
    assert.equal(chunked, 413);
  });

  test('wrong method answers 405, unknown path 404', async () => {
    const get = await fetch(`${base}/v1/activate`);
    assert.equal(get.status, 405);
    assert.equal(get.headers.get('allow'), 'POST');
    assert.deepEqual(await get.json(), { error: 'method_not_allowed' });
    const put = await fetch(`${base}/v1/validate`, { method: 'PUT', body: '{}' });
    assert.equal(put.status, 405);
    for (const route of ['/', '/v1/activate/', '/v2/activate', '/v1/revoke']) {
      const r = await post(route, {});
      assert.equal(r.status, 404, route);
      assert.deepEqual(r.body, { error: 'not_found' });
    }
    assert.equal((await post('/v1/activate?x=1', {})).status, 400, 'query string is ignored');
  });

  test('logs contain no license ids, machine ids, emails or addresses', async () => {
    const id = await newLicense();
    await activate(id, machine(7));
    await post('/v1/nope', {});
    const text = JSON.stringify(logs);
    assert.ok(logs.length > 0);
    assert.ok(!text.includes(id));
    assert.ok(!text.includes(machine(7)));
    assert.ok(!text.includes('@'));
    assert.ok(!text.includes('127.0.0.1'));
    assert.ok(!text.includes('/v1/nope'));
    for (const entry of logs) {
      for (const k of Object.keys(entry)) assert.ok(['route', 'method', 'status', 'ms', 'error'].includes(k), k);
    }
  });

  test('store failure answers 500 internal', async () => {
    const failing = {
      async get() {
        throw new Error('disk on fire');
      },
      async put() {},
    };
    const errLogs = [];
    const s = http.createServer(createHandler({ store: failing, key, log: (e) => errLogs.push(e) }));
    await new Promise((resolve) => s.listen(0, '127.0.0.1', resolve));
    try {
      const res = await fetch(`http://127.0.0.1:${s.address().port}/v1/activate`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ license_id: '6f1c2d3e-4b5a-4c6d-9e8f-0a1b2c3d4e5f', machine: machine(1), app_version: '1' }),
      });
      assert.equal(res.status, 500);
      assert.deepEqual(await res.json(), { error: 'internal' });
      assert.deepEqual(errLogs.map((e) => [e.status, e.error]), [[500, 'Error']]);
    } finally {
      await new Promise((resolve) => s.close(resolve));
    }
  });
});
