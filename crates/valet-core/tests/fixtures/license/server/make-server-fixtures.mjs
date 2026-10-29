// Fixtures signed by the LICENSE SERVICE's own code (services/license/lib), for the
// cross-check in crates/valet-core/tests/license_cross.rs: the service signer and the
// core verifier were written independently, so the core must accept what the service
// issues and reject it after one-byte tampering.
//
//   node crates/valet-core/tests/fixtures/license/server/make-server-fixtures.mjs [out-dir]
//
// Output is byte-identical on every run: the key comes from a fixed seed, Ed25519 is
// deterministic, the service clock is injected and every timestamp is a constant.
// The activation token goes through the real HTTP handler (createHandler) on
// 127.0.0.1, so it is exactly what POST /v1/activate answers.
//
// TEST key: the seed is committed on purpose. It signs only these fixtures; its
// public half is never shipped (crates/valet-core/license/pubkey.txt is Vlad's key).

import { createHash } from 'node:crypto';
import http from 'node:http';
import { mkdirSync, writeFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const HERE = dirname(fileURLToPath(import.meta.url));
const LIB = join(HERE, '../../../../../../services/license/lib/');
const { keyPairFromSeed, publicKeyFile } = await import(join(LIB, 'signing.mjs'));
const { issueLicense } = await import(join(LIB, 'license.mjs'));
const { createMemoryStore, registerLicense } = await import(join(LIB, 'store.mjs'));
const { createHandler } = await import(join(LIB, 'handler.mjs'));

const OUT = resolve(process.argv[2] ?? HERE);

// Keep in sync with crates/valet-core/tests/license_cross.rs.
const ISSUED_AT = 1_795_000_000;
const CHECKED_AT = 1_795_100_000;
const LICENSE_ID = '3e9a5c1d-7b2f-4c8e-9a6d-1f0e2d3c4b5a';
const MACHINE = 'a1b2c3d4e5f60718293a4b5c6d7e8f90';
const EMAIL = 'server-fixture@example.com';

const sha256 = (s) => createHash('sha256').update(s).digest();
const KEY = keyPairFromSeed(
  sha256('valet TEST server license key 1'),
  sha256('valet TEST server key id 1').subarray(0, 8),
);

const { payload, envelope: license } = issueLicense(
  { email: EMAIL, licenseId: LICENSE_ID, issuedAt: ISSUED_AT },
  KEY,
);

// Activation through the service's HTTP handler, clock pinned to CHECKED_AT.
const store = createMemoryStore();
await registerLicense(store, payload);
const server = http.createServer(
  createHandler({ store, key: KEY, now: () => CHECKED_AT, log: () => {} }),
);
await new Promise((r) => server.listen(0, '127.0.0.1', r));
let token;
try {
  const res = await fetch(`http://127.0.0.1:${server.address().port}/v1/activate`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ license_id: LICENSE_ID, machine: MACHINE, app_version: '0.2.0' }),
  });
  if (res.status !== 200) throw new Error(`activate answered ${res.status}`);
  token = (await res.json()).token;
} finally {
  await new Promise((r) => server.close(r));
}

const json = (v) => `${JSON.stringify(v, null, 2)}\n`;
const files = {
  'TEST-server-pubkey.txt': publicKeyFile(KEY),
  'server-license.valet-license': json(license),
  'server-token.valet-token': json(token),
};
mkdirSync(OUT, { recursive: true });
for (const [name, text] of Object.entries(files)) writeFileSync(join(OUT, name), text);
console.log(`wrote ${Object.keys(files).length} files to ${OUT}`);
