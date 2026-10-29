#!/usr/bin/env node
// Marks a license as revoked (refund, chargeback). Nothing is deleted; activate and
// validate answer 410 from now on.
//   node services/license/revoke.mjs --license <uuid> [--store path.json]

import { parseArgs } from 'node:util';
import { createJsonFileStore } from './lib/store.mjs';
import { UUID_V4 } from './lib/license.mjs';

function fail(message) {
  process.stderr.write(`revoke: ${message}\n`);
  process.exit(2);
}

let args;
try {
  args = parseArgs({
    options: { license: { type: 'string' }, store: { type: 'string' } },
    strict: true,
  }).values;
} catch (err) {
  fail(err.message);
}

const id = args.license;
if (!id || !UUID_V4.test(id)) fail('--license must be a lowercase UUID v4');
const storePath = args.store ?? process.env.VALET_LICENSE_STORE;
if (!storePath) fail('--store or VALET_LICENSE_STORE is required');

const store = createJsonFileStore(storePath);
const record = await store.get(id);
if (!record) fail('license not found in the store');
if (record.revoked) {
  process.stderr.write('revoke: already revoked\n');
} else {
  record.revoked = true;
  record.revoked_at = Math.floor(Date.now() / 1000);
  await store.put(id, record);
  process.stderr.write(`revoke: license ${id} revoked\n`);
}
