#!/usr/bin/env node
// Support: sends the license of an order again (the buyer lost the email).
//   node services/license/resend.mjs --order <txn_…|cs_…|pi_…> [--store path.json]
// The store has no email: it is read from the merchant (Paddle customer API, or the Stripe
// checkout session). The license is the same (same id and dates, signed again); with an
// unchanged email the file is byte-identical to the original. Refused for revoked and
// disputed licenses.
// Environment: VALET_LICENSE_SECRET_KEY, VALET_LICENSE_KEY_ID, VALET_RESEND_API_KEY,
// VALET_MAIL_FROM, and VALET_PADDLE_API_KEY (+ VALET_PADDLE_SANDBOX=1 in the sandbox) or
// VALET_STRIPE_API_KEY. Exit 0 sent, 1 merchant or Resend failure, 2 refused.

import { parseArgs } from 'node:util';
import { createJsonFileStore } from './lib/store.mjs';
import { nowSeconds } from './lib/license.mjs';
import { loadMailerConfig } from './lib/config.mjs';
import { orderKeyFor, merchantOf } from './lib/support.mjs';
import { resendLicense, ActionRefused, ActionFailed } from './lib/actions.mjs';

function fail(message, code = 2) {
  process.stderr.write(`resend: ${message}\n`);
  process.exit(code);
}

let args;
try {
  args = parseArgs({ options: { order: { type: 'string' }, store: { type: 'string' } }, strict: true }).values;
} catch (err) {
  fail(err.message);
}
if (args.order === undefined) fail('--order is required');
const orderKey = orderKeyFor(args.order);
if (!orderKey) fail('--order must be a Paddle transaction id (txn_…) or a Stripe checkout session (cs_…) or payment intent (pi_…)');
const storePath = args.store ?? process.env.VALET_LICENSE_STORE;
if (!storePath) fail('--store or VALET_LICENSE_STORE is required');

let config;
try {
  config = loadMailerConfig(process.env, merchantOf(orderKey));
} catch (err) {
  fail(err.message);
}

try {
  const r = await resendLicense({ store: createJsonFileStore(storePath), orderId: args.order, now: nowSeconds, ...config });
  process.stderr.write(`resend: license ${r.licenseId} sent to the buyer's email at the merchant${r.firstEmail ? ' (first successful email for it)' : ''}\n`);
} catch (err) {
  if (err instanceof ActionRefused) fail(err.message, 2);
  if (err instanceof ActionFailed) fail(err.message, 1);
  throw err;
}
