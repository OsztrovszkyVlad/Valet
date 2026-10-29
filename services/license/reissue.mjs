#!/usr/bin/env node
// Support: replaces a leaked license under the same order.
//   node services/license/reissue.mjs --order <txn_…|cs_…|pi_…> [--confirm] [--store path.json]
// Without --confirm it only says what it would do. With --confirm: revokes the old license
// (reason "reissued"), issues a new one (new license id, same updates_until, 3 activations),
// maps the order to it (the old id stays in the order's history), reads the buyer's email
// from the merchant and sends the new file. Refused for revoked and disputed licenses.
// If the email fails, the new license stays mapped: run resend.mjs --order <same id>.
// Environment: as resend.mjs. Exit 0 done, 1 merchant or Resend failure, 2 refused.

import { parseArgs } from 'node:util';
import { createJsonFileStore } from './lib/store.mjs';
import { nowSeconds } from './lib/license.mjs';
import { loadMailerConfig } from './lib/config.mjs';
import { orderKeyFor, merchantOf } from './lib/support.mjs';
import { reissueLicense, describeReissue, ActionRefused, ActionFailed } from './lib/actions.mjs';

function fail(message, code = 2) {
  process.stderr.write(`reissue: ${message}\n`);
  process.exit(code);
}

let args;
try {
  args = parseArgs({
    options: { order: { type: 'string' }, store: { type: 'string' }, confirm: { type: 'boolean', default: false } },
    strict: true,
  }).values;
} catch (err) {
  fail(err.message);
}
if (args.order === undefined) fail('--order is required');
const orderKey = orderKeyFor(args.order);
if (!orderKey) fail('--order must be a Paddle transaction id (txn_…) or a Stripe checkout session (cs_…) or payment intent (pi_…)');
const storePath = args.store ?? process.env.VALET_LICENSE_STORE;
if (!storePath) fail('--store or VALET_LICENSE_STORE is required');
const store = createJsonFileStore(storePath);

try {
  if (!args.confirm) fail(`${await describeReissue({ store, orderId: args.order })}. Nothing changed; run again with --confirm.`);
  let config;
  try {
    config = loadMailerConfig(process.env, merchantOf(orderKey));
  } catch (err) {
    fail(err.message);
  }
  const r = await reissueLicense({ store, orderId: args.order, confirm: true, now: nowSeconds, ...config });
  process.stderr.write(`reissue: license ${r.oldId} revoked (reissued); new license ${r.newId} mapped to the order\n`);
  if (!r.emailed) fail(`email not sent (${r.error}); run resend.mjs --order ${args.order} to send the new license`, 1);
  process.stderr.write("reissue: new license sent to the buyer's email at the merchant\n");
} catch (err) {
  if (err instanceof ActionRefused) fail(err.message, 2);
  if (err instanceof ActionFailed) fail(err.message, 1);
  throw err;
}
