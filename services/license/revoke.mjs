#!/usr/bin/env node
// Marks a license as revoked (refund, chargeback). Nothing is deleted; activate and
// validate answer 410 from now on.
//   node services/license/revoke.mjs --license <uuid> [--store path.json]
//   node services/license/revoke.mjs --order <txn_…|cs_…|pi_…> [--store path.json]
// Support uses --order (the order id from the buyer's receipt), never asks for the license id.

import { parseArgs } from 'node:util';
import { createJsonFileStore } from './lib/store.mjs';
import { resolveLicenseId } from './lib/support.mjs';

function fail(message) {
  process.stderr.write(`revoke: ${message}\n`);
  process.exit(2);
}

let args;
try {
  args = parseArgs({
    options: { license: { type: 'string' }, order: { type: 'string' }, store: { type: 'string' } },
    strict: true,
  }).values;
} catch (err) {
  fail(err.message);
}

const storePath = args.store ?? process.env.VALET_LICENSE_STORE;
if (!storePath) fail('--store or VALET_LICENSE_STORE is required');

const store = createJsonFileStore(storePath);
let id;
try {
  id = await resolveLicenseId(store, { license: args.license, order: args.order });
} catch (err) {
  fail(err.message);
}
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
