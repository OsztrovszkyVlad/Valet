#!/usr/bin/env node
// Support: shows the license behind a merchant order id from the buyer's receipt.
//   node services/license/lookup.mjs --order <txn_…|cs_…|pi_…> [--store path.json]
// Prints the license id, dates, revoked flag, active machine count and last-seen dates.
// The store holds no email, so none is printed. Never post this output in a public issue:
// the license id alone is enough to activate.

import { parseArgs } from 'node:util';
import { createJsonFileStore } from './lib/store.mjs';
import { resolveLicenseId, describeRecord } from './lib/support.mjs';

function fail(message) {
  process.stderr.write(`lookup: ${message}\n`);
  process.exit(2);
}

let args;
try {
  args = parseArgs({
    options: { order: { type: 'string' }, store: { type: 'string' } },
    strict: true,
  }).values;
} catch (err) {
  fail(err.message);
}

if (args.order === undefined) fail('--order is required');
const storePath = args.store ?? process.env.VALET_LICENSE_STORE;
if (!storePath) fail('--store or VALET_LICENSE_STORE is required');

const store = createJsonFileStore(storePath);
let id;
try {
  id = await resolveLicenseId(store, { order: args.order });
} catch (err) {
  fail(err.message);
}
const record = await store.get(id);
if (!record) fail('the order points to a license that is not in the store');
process.stdout.write(`${describeRecord(record)}\n`);
