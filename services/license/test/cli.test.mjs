import { test } from 'node:test';
import assert from 'node:assert/strict';
import { execFileSync, spawnSync } from 'node:child_process';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { openEnvelope } from '../lib/license.mjs';

const root = path.dirname(path.dirname(fileURLToPath(import.meta.url)));
const run = (script, args, env = {}) =>
  spawnSync(process.execPath, [path.join(root, script), ...args], {
    encoding: 'utf8',
    env: { PATH: process.env.PATH, ...env },
  });

function keygen() {
  const out = execFileSync(process.execPath, [path.join(root, 'keygen.mjs')], { encoding: 'utf8' });
  const lines = out.split('\n');
  const env = Object.fromEntries(
    lines.filter((l) => l.startsWith('VALET_LICENSE_')).map((l) => [l.slice(0, l.indexOf('=')), l.slice(l.indexOf('=') + 1)]),
  );
  const pub = lines.find((l) => l.startsWith('RW'));
  return { env, pub, out };
}

test('keygen prints a public key line and the two env values', () => {
  const { env, pub } = keygen();
  assert.match(pub, /^RW[A-Za-z0-9+/]{54}$/);
  assert.match(env.VALET_LICENSE_KEY_ID, /^[0-9a-f]{16}$/);
  assert.ok(env.VALET_LICENSE_SECRET_KEY.length > 40);
});

test('issue prints a license file and registers it; revoke flags it', () => {
  const { env, pub } = keygen();
  const store = path.join(fs.mkdtempSync(path.join(os.tmpdir(), 'valet-license-cli-')), 'store.json');
  const r = run('issue.mjs', ['--email', 't@example.com', '--days', '30', '--store', store], env);
  assert.equal(r.status, 0, r.stderr);
  const envelope = JSON.parse(r.stdout);
  assert.deepEqual(Object.keys(envelope), ['payload', 'signature']);
  assert.equal(typeof envelope.payload, 'string');
  assert.equal(envelope.signature.trimEnd().split('\n').length, 4);
  const payload = openEnvelope(envelope, pub);
  assert.equal(payload.email, 't@example.com');
  assert.equal(payload.updates_until - payload.issued_at, 30 * 86_400);
  const data = JSON.parse(fs.readFileSync(store, 'utf8'));
  assert.equal(data.licenses[payload.license_id].revoked, false);
  assert.ok(!r.stderr.includes('t@example.com'));

  const rv = run('revoke.mjs', ['--license', payload.license_id, '--store', store]);
  assert.equal(rv.status, 0, rv.stderr);
  assert.equal(JSON.parse(fs.readFileSync(store, 'utf8')).licenses[payload.license_id].revoked, true);
});

test('issue fails without key, store or email, and never echoes the secret', () => {
  const { env } = keygen();
  const store = path.join(os.tmpdir(), `valet-license-unused-${process.pid}.json`);
  assert.equal(run('issue.mjs', ['--email', 'a@b.co', '--store', store]).status, 2);
  assert.equal(run('issue.mjs', ['--email', 'a@b.co'], env).status, 2);
  assert.equal(run('issue.mjs', ['--store', store], env).status, 2);
  const bad = run('issue.mjs', ['--email', 'a@b.co', '--store', store], {
    ...env,
    VALET_LICENSE_SECRET_KEY: `${env.VALET_LICENSE_SECRET_KEY.slice(0, 20)}AAAA`,
  });
  assert.equal(bad.status, 2);
  assert.ok(!bad.stderr.includes(env.VALET_LICENSE_SECRET_KEY.slice(0, 20)));
  assert.ok(!fs.existsSync(store));
});

test('reset clears active machines, keeps history, and frees the slots', async () => {
  const { createJsonFileStore } = await import('../lib/store.mjs');
  const { createHandler } = await import('../lib/handler.mjs');
  const { loadSecretKeyFromEnv } = await import('../lib/signing.mjs');
  const http = await import('node:http');
  const { env } = keygen();
  const store = path.join(fs.mkdtempSync(path.join(os.tmpdir(), 'valet-license-reset-')), 'store.json');
  const issued = run('issue.mjs', ['--email', 't@example.com', '--store', store], env);
  assert.equal(issued.status, 0, issued.stderr);
  const id = JSON.parse(JSON.parse(issued.stdout).payload).license_id;

  const server = http.createServer(
    createHandler({ store: createJsonFileStore(store), key: loadSecretKeyFromEnv(env), log: () => {} }),
  );
  await new Promise((resolve) => server.listen(0, '127.0.0.1', resolve));
  const activate = async (n) => {
    const res = await fetch(`http://127.0.0.1:${server.address().port}/v1/activate`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ license_id: id, machine: n.toString(16).padStart(32, '0'), app_version: '0.2.0' }),
    });
    await res.text();
    return res.status;
  };
  try {
    for (const n of [1, 2, 3]) assert.equal(await activate(n), 200);
    assert.equal(await activate(4), 409);

    const r = run('reset.mjs', ['--license', id, '--store', store]);
    assert.equal(r.status, 0, r.stderr);
    assert.match(r.stderr, /3 active machine\(s\) cleared/);
    const rec = JSON.parse(fs.readFileSync(store, 'utf8')).licenses[id];
    assert.equal(Object.keys(rec.machines).length, 3, 'history kept');
    for (const m of Object.values(rec.machines)) {
      assert.equal(m.active, false);
      assert.equal(typeof m.first_seen, 'number');
      assert.equal(typeof m.deactivated_at, 'number');
    }
    assert.equal(rec.resets.length, 1);
    assert.equal(rec.resets[0].cleared, 3);
    assert.equal(rec.revoked, false);

    assert.equal(await activate(4), 200, 'slot freed by reset');
    assert.equal(run('reset.mjs', ['--license', id, '--store', store]).status, 0);
    assert.equal(run('reset.mjs', ['--license', 'nope', '--store', store]).status, 2);
    assert.equal(
      run('reset.mjs', ['--license', '6f1c2d3e-4b5a-4c6d-9e8f-0a1b2c3d4e5f', '--store', store]).status,
      2,
      'unknown license',
    );
  } finally {
    await new Promise((resolve) => server.close(resolve));
  }
});
