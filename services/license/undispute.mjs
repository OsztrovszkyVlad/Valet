#!/usr/bin/env node
// Manual end of a chargeback or dispute (the only way back for a disputed license).
//   node services/license/undispute.mjs --order <txn_…|cs_…|pi_…> --confirm [--by <name>] [--store path.json]
//   node services/license/undispute.mjs --license <uuid> --confirm [--by <name>] [--store path.json]
// Clears `disputed`, and `revoked` when the dispute was the reason (a license revoked for a
// refund or a reissue stays revoked). Appends { at, action: "undispute", by? } to the
// license history. Sends no email. Without --confirm it only says what it would do.

import { parseArgs } from 'node:util';
import { createJsonFileStore } from './lib/store.mjs';
import { nowSeconds } from './lib/license.mjs';
import { resolveLicenseId } from './lib/support.mjs';
import { undisputeLicense, ActionRefused } from './lib/actions.mjs';

function fail(message) {
  process.stderr.write(`undispute: ${message}\n`);
  process.exit(2);
}

let args;
try {
  args = parseArgs({
    options: {
      license: { type: 'string' },
      order: { type: 'string' },
      store: { type: 'string' },
      by: { type: 'string' },
      confirm: { type: 'boolean', default: false },
    },
    strict: true,
  }).values;
} catch (err) {
  fail(err.message);
}
if (args.by !== undefined && !/^[A-Za-z0-9._-]{1,32}$/.test(args.by)) fail('--by must be 1-32 characters of A-Z a-z 0-9 . _ - (a name, not an email)');
const storePath = args.store ?? process.env.VALET_LICENSE_STORE;
if (!storePath) fail('--store or VALET_LICENSE_STORE is required');

const store = createJsonFileStore(storePath);
let id;
try {
  id = await resolveLicenseId(store, { license: args.license, order: args.order });
} catch (err) {
  fail(err.message);
}
try {
  const r = await undisputeLicense({ store, licenseId: id, confirm: args.confirm, now: nowSeconds, by: args.by });
  if (!r.changed) {
    process.stderr.write(`undispute: license ${id} is not disputed; nothing changed${r.revoked ? ` (it is revoked: ${r.revokedReason ?? 'by hand'})` : ''}\n`);
  } else if (r.revoked) {
    process.stderr.write(`undispute: license ${id} no longer disputed; it stays revoked (${r.revokedReason ?? 'by hand'})\n`);
  } else {
    process.stderr.write(`undispute: license ${id} no longer disputed and active again\n`);
  }
} catch (err) {
  if (err instanceof ActionRefused) fail(err.message);
  throw err;
}
