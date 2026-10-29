import { test } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { createJsonFileStore, registerLicense } from '../lib/store.mjs';
import { buildLicensePayload } from '../lib/license.mjs';

const tmp = () => fs.mkdtempSync(path.join(os.tmpdir(), 'valet-license-store-'));

test('json store: missing file reads as empty, put persists, no temp files left', async () => {
  const dir = tmp();
  const file = path.join(dir, 'nested', 'store.json');
  const store = createJsonFileStore(file);
  assert.equal(await store.get('x'), null);
  const payload = buildLicensePayload({ email: 'a@b.co', issuedAt: 100 });
  await registerLicense(store, payload);
  const again = createJsonFileStore(file);
  const rec = await again.get(payload.license_id);
  assert.deepEqual(rec, {
    license_id: payload.license_id,
    max_activations: 3,
    issued_at: 100,
    updates_until: 100 + 365 * 86_400,
    revoked: false,
    machines: {},
  });
  assert.ok(!JSON.stringify(rec).includes('a@b.co'), 'email is not stored');
  assert.deepEqual(fs.readdirSync(path.dirname(file)), ['store.json']);
  if (process.platform !== 'win32') assert.equal(fs.statSync(file).mode & 0o777, 0o600);
});

test('json store: registering the same id twice is refused', async () => {
  const store = createJsonFileStore(path.join(tmp(), 's.json'));
  const payload = buildLicensePayload({ email: 'a@b.co' });
  await registerLicense(store, payload);
  await assert.rejects(registerLicense(store, payload), /already registered/);
});

test('json store: concurrent puts in one process are all kept', async () => {
  const store = createJsonFileStore(path.join(tmp(), 's.json'));
  await Promise.all(Array.from({ length: 20 }, (_, i) => store.put(`id${i}`, { n: i })));
  for (let i = 0; i < 20; i++) assert.deepEqual(await store.get(`id${i}`), { n: i });
});

test('json store: a corrupt file is never overwritten', async () => {
  const file = path.join(tmp(), 's.json');
  fs.writeFileSync(file, '{broken');
  const store = createJsonFileStore(file);
  await assert.rejects(store.put('a', {}));
  await assert.rejects(store.get('a'));
  assert.equal(fs.readFileSync(file, 'utf8'), '{broken');
});
