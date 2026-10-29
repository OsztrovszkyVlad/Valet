import { test } from 'node:test';
import assert from 'node:assert/strict';
import { spawnSync, execFileSync } from 'node:child_process';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { orderKeyFor } from '../lib/support.mjs';

const root = path.dirname(path.dirname(fileURLToPath(import.meta.url)));
const run = (script, args, env = {}) =>
  spawnSync(process.execPath, [path.join(root, script), ...args], { encoding: 'utf8', env: { PATH: process.env.PATH, ...env } });

const TXN = 'txn_01jbx3dq8w5h2k9m4n7p0r3t6v';
const CS = 'cs_test_a1B2c3D4e5F6g7H8i9J0k1L2m3N4o5P6q7R8s9T0u1V2w3X4y5Z6';
const PI = 'pi_3QBx7mE5valet0000000001';

/** A store with one Paddle license (2 active machines, 1 inactive), one Stripe license, one refund-only order. */
function makeStore() {
  const out = execFileSync(process.execPath, [path.join(root, 'keygen.mjs')], { encoding: 'utf8' });
  const env = Object.fromEntries(
    out.split('\n').filter((l) => l.startsWith('VALET_LICENSE_')).map((l) => [l.slice(0, l.indexOf('=')), l.slice(l.indexOf('=') + 1)]),
  );
  const store = path.join(fs.mkdtempSync(path.join(os.tmpdir(), 'valet-license-support-')), 'store.json');
  const ids = [];
  for (let i = 0; i < 2; i++) {
    const r = run('issue.mjs', ['--email', 'buyer@example.com', '--store', store], env);
    assert.equal(r.status, 0, r.stderr);
    ids.push(JSON.parse(JSON.parse(r.stdout).payload).license_id);
  }
  const data = JSON.parse(fs.readFileSync(store, 'utf8'));
  const m = (n) => n.toString(16).padStart(32, '0');
  data.licenses[ids[0]].machines = {
    [m(1)]: { first_seen: 1793000000, last_seen: 1795000000, active: true, deactivated_at: null },
    [m(2)]: { first_seen: 1793000000, last_seen: 1794000000, active: true, deactivated_at: null },
    [m(3)]: { first_seen: 1793000000, last_seen: 1793500000, active: false, deactivated_at: 1793500000 },
  };
  data.licenses[ids[0]].emailed_at = 1793000001;
  data.orders = {
    [`paddle:${TXN}`]: { license_id: ids[0], created_at: 1793000000 },
    [`stripe:${CS}`]: { license_id: ids[1], created_at: 1793000000 },
    [`stripe:${PI}`]: { license_id: ids[1], created_at: 1793000000 },
    'paddle:txn_01refundonly0000000000000': { license_id: null, refunded_at: 1793000000 },
  };
  fs.writeFileSync(store, JSON.stringify(data));
  return { store, ids, read: () => JSON.parse(fs.readFileSync(store, 'utf8')), machine: m };
}

test('order ids map to store keys by prefix', () => {
  assert.equal(orderKeyFor(TXN), `paddle:${TXN}`);
  assert.equal(orderKeyFor(CS), `stripe:${CS}`);
  assert.equal(orderKeyFor(PI), `stripe:${PI}`);
  assert.equal(orderKeyFor(` ${TXN} `), `paddle:${TXN}`, 'pasted with spaces');
  for (const bad of ['', 'ch_123', 'TXN_01abc', 'txn_01ABC', 'adj_01abc', '6f1c2d3e-4b5a-4c6d-9e8f-0a1b2c3d4e5f', undefined]) {
    assert.equal(orderKeyFor(bad), null, String(bad));
  }
});

test('lookup --order prints license, dates, revoked flag, machines; no email or machine ids', () => {
  const s = makeStore();
  const r = run('lookup.mjs', ['--order', TXN, '--store', s.store]);
  assert.equal(r.status, 0, r.stderr);
  assert.match(r.stdout, new RegExp(`^license_id: ${s.ids[0]}$`, 'm'));
  assert.match(r.stdout, /^issued: \d{4}-\d{2}-\d{2}$/m);
  assert.match(r.stdout, /^updates_until: \d{4}-\d{2}-\d{2}$/m);
  assert.match(r.stdout, /^revoked: no$/m);
  assert.match(r.stdout, /^emailed: 2026-/m);
  assert.match(r.stdout, /^active_machines: 2 of 3$/m);
  assert.match(r.stdout, /^machine 1: active, first seen 2026-10-26, last seen 2026-11-18$/m);
  assert.match(r.stdout, /^machine 3: inactive, /m);
  assert.ok(!r.stdout.includes('@'));
  assert.ok(!r.stdout.includes(s.machine(1)));

  for (const order of [CS, PI]) {
    const st = run('lookup.mjs', ['--order', order, '--store', s.store]);
    assert.equal(st.status, 0, st.stderr);
    assert.match(st.stdout, new RegExp(`license_id: ${s.ids[1]}`));
  }
});

test('lookup fails clearly for unknown, malformed, refund-only orders and missing arguments', () => {
  const s = makeStore();
  const unknown = run('lookup.mjs', ['--order', 'txn_01unknown000000000000000', '--store', s.store]);
  assert.equal(unknown.status, 2);
  assert.match(unknown.stderr, /order not found/);
  const malformed = run('lookup.mjs', ['--order', s.ids[0], '--store', s.store]);
  assert.equal(malformed.status, 2, 'a license id is not an order id');
  assert.match(malformed.stderr, /--order must be/);
  const refundOnly = run('lookup.mjs', ['--order', 'txn_01refundonly0000000000000', '--store', s.store]);
  assert.equal(refundOnly.status, 2);
  assert.match(refundOnly.stderr, /order has no license/);
  assert.equal(run('lookup.mjs', ['--store', s.store]).status, 2);
  assert.equal(run('lookup.mjs', ['--order', TXN]).status, 2, 'no store');
});

test('reset --order clears the machines of the license behind the order', () => {
  const s = makeStore();
  const r = run('reset.mjs', ['--order', TXN, '--store', s.store]);
  assert.equal(r.status, 0, r.stderr);
  assert.match(r.stderr, /2 active machine\(s\) cleared/);
  const rec = s.read().licenses[s.ids[0]];
  assert.equal(Object.values(rec.machines).filter((m) => m.active).length, 0);
  assert.equal(rec.resets.length, 1);
  assert.equal(s.read().licenses[s.ids[1]].resets, undefined, 'other license untouched');
});

test('revoke --order revokes through a Stripe payment intent; bad combinations fail', () => {
  const s = makeStore();
  const r = run('revoke.mjs', ['--order', PI, '--store', s.store]);
  assert.equal(r.status, 0, r.stderr);
  assert.equal(s.read().licenses[s.ids[1]].revoked, true);
  assert.equal(s.read().licenses[s.ids[0]].revoked, false);
  for (const script of ['reset.mjs', 'revoke.mjs']) {
    assert.equal(run(script, ['--order', TXN, '--license', s.ids[0], '--store', s.store]).status, 2, `${script}: both`);
    assert.equal(run(script, ['--store', s.store]).status, 2, `${script}: neither`);
    assert.equal(run(script, ['--order', 'txn_01unknown000000000000000', '--store', s.store]).status, 2, `${script}: unknown order`);
    assert.equal(run(script, ['--order', 'nope', '--store', s.store]).status, 2, `${script}: malformed`);
  }
  assert.equal(run('revoke.mjs', ['--license', s.ids[0], '--store', s.store]).status, 0, '--license still works');
});
