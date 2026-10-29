#!/usr/bin/env node
// Marks a license as revoked by hand (e.g. a refund made outside the webhook). Nothing is
// deleted; activate and validate answer 410 from now on. Chargebacks and disputes are
// revoked by the webhook (disputed); a leaked license is replaced with reissue.mjs.
// Appends { at, action: "revoke", reason: "manual" } to the license history.
//   node services/license/revoke.mjs --license <uuid> [--store path.json]
//   node services/license/revoke.mjs --order <txn_…|cs_…|pi_…> [--store path.json]
// Support uses --order (the order id from the buyer's receipt), never asks for the license id.

import { parseArgs } from 'node:util';
import { createJsonFileStore, updateRecord, addHistory } from './lib/store.mjs';
import { nowSeconds } from './lib/license.mjs';
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
const t = nowSeconds();
const result = await updateRecord(store, id, (record) => {
  if (!record) return 'missing';
  if (record.revoked) return 'already';
  record.revoked = true;
  record.revoked_at = t;
  addHistory(record, { at: t, action: 'revoke', reason: 'manual' });
  return 'revoked';
});
if (result === 'missing') fail('license not found in the store');
process.stderr.write(result === 'already' ? 'revoke: already revoked\n' : `revoke: license ${id} revoked\n`);
