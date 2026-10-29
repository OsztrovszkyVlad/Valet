#!/usr/bin/env node
// Support reset: clears the active machines of a license (the old PC is gone and can't
// deactivate). Machine history and the record are kept; the user activates again.
//   node services/license/reset.mjs --license <uuid> [--store path.json]
//   node services/license/reset.mjs --order <txn_…|cs_…|pi_…> [--store path.json]
// Support uses --order (the order id from the buyer's receipt), never asks for the license id.

import { parseArgs } from 'node:util';
import { createJsonFileStore, resetActiveMachines } from './lib/store.mjs';
import { nowSeconds } from './lib/license.mjs';
import { resolveLicenseId } from './lib/support.mjs';

function fail(message) {
  process.stderr.write(`reset: ${message}\n`);
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
const cleared = resetActiveMachines(record, nowSeconds());
if (cleared > 0) await store.put(id, record);
process.stderr.write(`reset: license ${id}, ${cleared} active machine(s) cleared\n`);
