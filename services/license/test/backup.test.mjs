import { test } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { backupStore, startBackups, backupDirFor } from '../lib/backup.mjs';

const tmp = () => fs.mkdtempSync(path.join(os.tmpdir(), 'valet-license-backup-'));
const storeText = (n) => `${JSON.stringify({ version: 1, licenses: { n: { n } } }, null, 2)}\n`;

test('backup creates a dated copy and never overwrites the day\'s file', async () => {
  const dir = tmp();
  const file = path.join(dir, 'store.json');
  fs.writeFileSync(file, storeText(1));
  const day = new Date('2026-10-29T03:00:00Z');
  const first = await backupStore(file, { now: day });
  assert.deepEqual(first, { result: 'created', name: 'store-2026-10-29.json', removed: 0 });
  const copy = path.join(dir, 'backups', 'store-2026-10-29.json');
  assert.equal(fs.readFileSync(copy, 'utf8'), storeText(1));
  if (process.platform !== 'win32') assert.equal(fs.statSync(copy).mode & 0o777, 0o600);

  fs.writeFileSync(file, storeText(2));
  const again = await backupStore(file, { now: new Date('2026-10-29T23:59:59Z') });
  assert.equal(again.result, 'exists');
  assert.equal(fs.readFileSync(copy, 'utf8'), storeText(1), 'first copy of the day kept');
  assert.deepEqual(fs.readdirSync(path.join(dir, 'backups')), ['store-2026-10-29.json'], 'no temp files left');

  const next = await backupStore(file, { now: new Date('2026-10-30T00:00:01Z') });
  assert.equal(next.result, 'created');
  assert.equal(fs.readFileSync(path.join(dir, 'backups', 'store-2026-10-30.json'), 'utf8'), storeText(2));
});

test('backup keeps the newest 30 dated copies and touches nothing else', async () => {
  const dir = tmp();
  const file = path.join(dir, 'store.json');
  fs.writeFileSync(file, storeText(1));
  const backups = backupDirFor(file);
  fs.mkdirSync(backups);
  const start = Date.parse('2026-09-01T12:00:00Z');
  for (let i = 0; i < 35; i++) {
    const d = new Date(start + i * 86_400_000).toISOString().slice(0, 10);
    fs.writeFileSync(path.join(backups, `store-${d}.json`), '{}');
  }
  const others = ['notes.txt', 'store-2026-1-1.json', 'store-2020-01-01.json.bak', 'other-2020-01-01.json'];
  for (const o of others) fs.writeFileSync(path.join(backups, o), 'x');
  fs.writeFileSync(path.join(dir, 'store-2020-01-01.json'), 'outside backups/');

  const r = await backupStore(file, { now: new Date('2026-10-29T12:00:00Z') });
  assert.equal(r.result, 'created');
  assert.equal(r.removed, 6);
  const left = fs.readdirSync(backups).sort();
  const dated = left.filter((n) => /^store-\d{4}-\d{2}-\d{2}\.json$/.test(n));
  assert.equal(dated.length, 30);
  assert.equal(dated[dated.length - 1], 'store-2026-10-29.json');
  assert.equal(dated[0], 'store-2026-09-07.json', 'oldest six removed');
  for (const o of others) assert.ok(left.includes(o), `${o} kept`);
  assert.ok(fs.existsSync(path.join(dir, 'store-2020-01-01.json')));
});

test('no store yet, or a store that does not parse: no copy', async () => {
  const dir = tmp();
  const file = path.join(dir, 'store.json');
  assert.equal((await backupStore(file)).result, 'no_store');
  fs.writeFileSync(file, '{broken');
  assert.equal((await backupStore(file)).result, 'invalid_store');
  assert.equal(fs.existsSync(path.join(dir, 'backups')), false);
});

test('startBackups runs at once and logs the result without paths', async () => {
  const dir = tmp();
  const file = path.join(dir, 'store.json');
  fs.writeFileSync(file, storeText(1));
  const logs = [];
  const b = startBackups({ storeFile: file, log: (e) => logs.push(e) });
  await b.first;
  b.stop();
  assert.deepEqual(logs, [{ backup: 'created', removed: 0 }]);
  assert.equal(fs.readdirSync(path.join(dir, 'backups')).length, 1);
});
