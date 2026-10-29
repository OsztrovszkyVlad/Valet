#!/usr/bin/env node
// Support reset: clears the active machines of a license (the old PC is gone and can't
// deactivate). Machine history and the record are kept; the user activates again.
//   node services/license/reset.mjs --license <uuid> [--store path.json]
//   node services/license/reset.mjs --order <txn_…|cs_…|pi_…> [--store path.json]
// Support uses --order (the order id from the buyer's receipt), never asks for the license id.
// A disputed license is refused (only undispute.mjs ends a dispute). A revoked license keeps
// its revocation: reset only clears machines.

import { parseArgs } from 'node:util';
import { createJsonFileStore, resetActiveMachines, updateRecord } from './lib/store.mjs';
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
const t = nowSeconds();
const result = await updateRecord(store, id, (record) => {
  if (!record) return { missing: true };
  if (record.disputed) return { disputed: true };
  return { cleared: resetActiveMachines(record, t), revoked: record.revoked === true, reason: record.revoked_reason };
});
if (result.missing) fail('license not found in the store');
if (result.disputed) {
  fail(`license ${id} is disputed (chargeback) and stays revoked; reset refused, nothing changed. Only undispute.mjs --confirm ends a dispute.`);
}
process.stderr.write(`reset: license ${id}, ${result.cleared} active machine(s) cleared\n`);
if (result.revoked) process.stderr.write(`reset: the license stays revoked (${result.reason ?? 'by hand'}); reset does not change that\n`);
