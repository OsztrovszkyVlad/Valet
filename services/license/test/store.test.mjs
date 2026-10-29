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

test('json store: two processes writing at once keep every entry', async () => {
  const { spawn } = await import('node:child_process');
  const file = path.join(tmp(), 's.json');
  const storeUrl = new URL('../lib/store.mjs', import.meta.url).href;
  const script = (tag) =>
    `const { createJsonFileStore } = await import(${JSON.stringify(storeUrl)});` +
    `const s = createJsonFileStore(${JSON.stringify(file)});` +
    `for (let i = 0; i < 25; i++) { await s.put('${tag}' + i, { i }); await s.putOrder('o${tag}' + i, { i }); }`;
  const runOne = (tag) =>
    new Promise((resolve, reject) => {
      const c = spawn(process.execPath, ['--input-type=module', '-e', script(tag)], { stdio: 'inherit' });
      c.on('exit', (code) => (code === 0 ? resolve() : reject(new Error(`exit ${code}`))));
    });
  await Promise.all([runOne('a'), runOne('b')]);
  const data = JSON.parse(fs.readFileSync(file, 'utf8'));
  assert.equal(Object.keys(data.licenses).length, 50);
  assert.equal(Object.keys(data.orders).length, 50);
  assert.deepEqual(fs.readdirSync(path.dirname(file)), ['s.json'], 'no lock or temp file left');
});

test('json store: a held lock makes writes wait; a stale lock is taken over', async () => {
  const dir = tmp();
  const file = path.join(dir, 's.json');
  const store = createJsonFileStore(file);
  fs.writeFileSync(`${file}.lock`, '');
  let done = false;
  const p = store.put('a', { n: 1 }).then(() => {
    done = true;
  });
  await new Promise((r) => setTimeout(r, 100));
  assert.equal(done, false, 'waits while the lock is held');
  fs.unlinkSync(`${file}.lock`);
  await p;
  assert.deepEqual(await store.get('a'), { n: 1 });

  fs.writeFileSync(`${file}.lock`, '');
  const old = new Date(Date.now() - 60_000);
  fs.utimesSync(`${file}.lock`, old, old);
  await store.putOrder('o', { license_id: null });
  assert.deepEqual(await store.getOrder('o'), { license_id: null });
  assert.equal(fs.existsSync(`${file}.lock`), false);
});

test('json store: written as root, the file keeps the owner of its directory', { skip: typeof process.getuid !== 'function' || process.getuid() !== 0 }, async () => {
  const dir = tmp();
  fs.chownSync(dir, 65534, 65534);
  const file = path.join(dir, 's.json');
  await createJsonFileStore(file).put('a', {});
  assert.equal(fs.statSync(file).uid, 65534);
  assert.equal(fs.statSync(file).gid, 65534);
});
