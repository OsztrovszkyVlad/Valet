// Cross-check, core -> service: the license fixtures of valet-core
// (crates/valet-core/tests/fixtures/license, made by make-fixtures.mjs, an independent
// signer) must verify with the service's own verify code, and the bad ones must fail.
// The other direction is crates/valet-core/tests/license_cross.rs.
// Also checks that both fixture generators are deterministic: a fresh run in a temp
// directory gives byte-identical files to the committed ones.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { verify, parsePublicKey } from '../lib/signing.mjs';
import { openEnvelope } from '../lib/license.mjs';

const repo = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../../..');
const FIX = path.join(repo, 'crates/valet-core/tests/fixtures/license');
const read = (name) => fs.readFileSync(path.join(FIX, name), 'utf8');
const TEST_KEY = read('TEST-pubkey.txt');
const OTHER_KEY = read('TEST-other-pubkey.txt');

const LICENSE_ID = '6f1c2a3b-4d5e-4f60-8a71-92b3c4d5e6f7';

// Signature valid with the TEST key (content checks are the core's job).
const VALID = [
  'license-valid.valet-license',
  'license-wrong-product.valet-license',
  'license-not-covering-build.valet-license',
  'license-v2.valet-license',
  'token-valid.valet-token',
  'token-31-days.valet-token',
  'token-61-days.valet-token',
  'token-future-2-days.valet-token',
  'token-other-machine.valet-token',
  'token-other-license.valet-token',
];
// Must fail with the TEST key, and why.
const INVALID = {
  'license-tampered.valet-license': /signature: invalid$/,
  'token-tampered.valet-token': /signature: invalid$/,
  'license-other-key.valet-license': /key id mismatch/,
  'token-other-key.valet-token': /key id mismatch/,
  'license-forged-same-key-id.valet-license': /signature: invalid$/,
  'license-legacy.valet-license': /legacy format not allowed/,
};

test('every committed core fixture is classified', () => {
  const envelopes = fs
    .readdirSync(FIX)
    .filter((f) => /\.valet-(license|token)$/.test(f) && f !== 'license-malformed.valet-license');
  assert.deepEqual(envelopes.sort(), [...VALID, ...Object.keys(INVALID)].sort());
});

test('core fixtures with a valid signature open with the service verifier', () => {
  for (const name of VALID) {
    const payload = openEnvelope(JSON.parse(read(name)), TEST_KEY);
    assert.ok(payload.kind === 'license' || payload.kind === 'activation', name);
    if (name === 'license-valid.valet-license') {
      assert.deepEqual(payload, {
        v: 1,
        kind: 'license',
        license_id: LICENSE_ID,
        product: 'valet-pro',
        issued_at: 1_790_000_000,
        updates_until: 1_821_536_000,
        max_activations: 3,
        email: 'test@example.com',
      });
    }
    if (name === 'token-valid.valet-token') {
      assert.deepEqual(payload, {
        v: 1,
        kind: 'activation',
        license_id: LICENSE_ID,
        machine: '0123456789abcdef0123456789abcdef',
        checked_at: 1_794_000_000 - 86_400,
      });
    }
  }
});

test('tampered, other-key, forged and legacy core fixtures fail in the service verifier', () => {
  for (const [name, why] of Object.entries(INVALID)) {
    assert.throws(() => openEnvelope(JSON.parse(read(name)), TEST_KEY), why, name);
  }
  // The other-key fixtures are well formed: they verify with their own key.
  openEnvelope(JSON.parse(read('license-other-key.valet-license')), OTHER_KEY);
  openEnvelope(JSON.parse(read('token-other-key.valet-token')), OTHER_KEY);
  // And a TEST-signed fixture fails with the other key.
  assert.throws(
    () => openEnvelope(JSON.parse(read('license-valid.valet-license')), OTHER_KEY),
    /key id mismatch/,
  );
  // Malformed JSON never reaches the verifier.
  assert.throws(() => JSON.parse(read('license-malformed.valet-license')), SyntaxError);
});

test('one-byte payload change of a core fixture fails in the service verifier', () => {
  const env = JSON.parse(read('license-valid.valet-license'));
  const key = parsePublicKey(TEST_KEY);
  const bytes = Buffer.from(env.payload, 'utf8');
  for (let i = 0; i < bytes.length; i += 1) {
    const b = Buffer.from(bytes);
    b[i] ^= 0x01;
    assert.throws(() => verify(b, env.signature, key), /signature: invalid$/, `byte ${i}`);
  }
});

test('server fixtures open with the service verifier and only with their key', () => {
  const key = read('server/TEST-server-pubkey.txt');
  const lic = openEnvelope(JSON.parse(read('server/server-license.valet-license')), key);
  const tok = openEnvelope(JSON.parse(read('server/server-token.valet-token')), key);
  assert.equal(lic.license_id, tok.license_id);
  assert.throws(
    () => openEnvelope(JSON.parse(read('server/server-license.valet-license')), TEST_KEY),
    /key id mismatch/,
  );
});

function sha(dir, names) {
  return Object.fromEntries(
    names.map((n) => [n, createHash('sha256').update(fs.readFileSync(path.join(dir, n))).digest('hex')]),
  );
}

function runNode(script, args = []) {
  const r = spawnSync(process.execPath, [script, ...args], { encoding: 'utf8' });
  assert.equal(r.status, 0, r.stderr);
}

test('server fixture generator is deterministic and matches the committed files', () => {
  const names = ['TEST-server-pubkey.txt', 'server-license.valet-license', 'server-token.valet-token'];
  const script = path.join(FIX, 'server/make-server-fixtures.mjs');
  const a = fs.mkdtempSync(path.join(os.tmpdir(), 'valet-srvfix-a-'));
  const b = fs.mkdtempSync(path.join(os.tmpdir(), 'valet-srvfix-b-'));
  runNode(script, [a]);
  runNode(script, [b]);
  assert.deepEqual(sha(a, names), sha(b, names));
  assert.deepEqual(sha(a, names), sha(path.join(FIX, 'server'), names));
  for (const d of [a, b]) fs.rmSync(d, { recursive: true, force: true });
});

test('core fixture generator is deterministic and matches the committed files', () => {
  // make-fixtures.mjs writes next to itself: run copies in two temp directories.
  const names = fs.readdirSync(FIX).filter((f) => /\.(txt|valet-license|valet-token)$/.test(f));
  const runs = [0, 1].map((i) => {
    const dir = fs.mkdtempSync(path.join(os.tmpdir(), `valet-corefix-${i}-`));
    fs.copyFileSync(path.join(FIX, 'make-fixtures.mjs'), path.join(dir, 'make-fixtures.mjs'));
    runNode(path.join(dir, 'make-fixtures.mjs'));
    const hashes = sha(dir, names);
    fs.rmSync(dir, { recursive: true, force: true });
    return hashes;
  });
  assert.deepEqual(runs[0], runs[1]);
  assert.deepEqual(runs[0], sha(FIX, names));
});
